use std::{collections::VecDeque, convert};

use eyre::{Report, Result};
use tokio::sync::mpsc;

use crate::{
    catalog::operation::convert_image::{
        apply_convert_image, perform_side_effects_convert_image, ConvertImage,
        ImageConversionSideEffectResult,
    },
    config,
    core::storage::Storage,
    model::repository::db::DbPool,
};

#[derive(Debug)]
pub enum MsgFromImageConversion {
    ActivityChange {
        running: usize,
        queued: usize,
    },
    DroppedMessage,
    ConversionComplete(ConvertImage),
    ConversionError {
        convert_image: ConvertImage,
        report: Report,
    },
}

#[derive(Clone)]
pub struct ImageConversionActorHandle {
    send: mpsc::UnboundedSender<MsgToImageConversion>,
}

#[derive(Debug, Clone)]
enum MsgToImageConversion {
    Pause,
    Resume,
    DoTask(DoTaskMsg),
}

#[derive(Debug, Clone)]
enum DoTaskMsg {
    ConvertImage(ConvertImage),
}

#[derive(Debug)]
struct ImageConversionResult {
    result: ImageConversionSideEffectResult,
    convert_image: ConvertImage,
}
type TaskResult = Result<ImageConversionResult>;

impl ImageConversionActorHandle {
    pub fn new(
        db_pool: DbPool,
        storage: Storage,
        config: config::Config,
        send_from_us: mpsc::UnboundedSender<MsgFromImageConversion>,
    ) -> Self {
        let (send, recv) = mpsc::unbounded_channel::<MsgToImageConversion>();
        let (task_result_send, task_result_recv) = mpsc::unbounded_channel::<TaskResult>();
        let actor = ImageConversionActor {
            db_pool,
            storage,
            config,
            send_from_us,
            task_result_send,
        };
        tokio::spawn(run_image_conversion_actor(recv, task_result_recv, actor));
        Self { send }
    }

    pub fn msg_convert_image(&self, convert_image: ConvertImage) -> Result<()> {
        self.send
            .send(MsgToImageConversion::DoTask(DoTaskMsg::ConvertImage(
                convert_image,
            )))?;
        Ok(())
    }

    pub fn msg_pause_all(&self) -> Result<()> {
        self.send.send(MsgToImageConversion::Pause)?;
        Ok(())
    }

    pub fn msg_resume_all(&self) -> Result<()> {
        self.send.send(MsgToImageConversion::Resume)?;
        Ok(())
    }
}

struct ImageConversionActor {
    db_pool: DbPool,
    storage: Storage,
    config: config::Config,
    send_from_us: mpsc::UnboundedSender<MsgFromImageConversion>,
    task_result_send: mpsc::UnboundedSender<TaskResult>,
}

const MAX_TASKS: usize = 4;
const MAX_QUEUE_SIZE: usize = 10;
async fn run_image_conversion_actor(
    mut actor_recv: mpsc::UnboundedReceiver<MsgToImageConversion>,
    mut task_result_recv: mpsc::UnboundedReceiver<TaskResult>,
    actor: ImageConversionActor,
) {
    let mut is_running = true;
    let mut running_tasks: usize = 0;
    let mut queue: VecDeque<DoTaskMsg> = Default::default();
    loop {
        tokio::select! {
            Some(msg) = actor_recv.recv() => {
                match msg {
                    MsgToImageConversion::Pause => {
                        is_running = false;
                    }
                    MsgToImageConversion::Resume => {
                        is_running = true;
                    }
                    MsgToImageConversion::DoTask(task) => {
                        if is_running && running_tasks < MAX_TASKS {
                            tracing::debug!(?task, "received msg, processing immediately");
                            running_tasks += 1;
                            let _ = actor.send_from_us.send(MsgFromImageConversion::ActivityChange {
                                running: running_tasks,
                                queued: queue.len(),
                            });
                            actor.process_message(task).await;
                        } else if queue.len() < MAX_QUEUE_SIZE {
                            tracing::debug!("received msg, queuing it");
                            queue.push_back(task);
                            let _ = actor.send_from_us.send(MsgFromImageConversion::ActivityChange {
                                running: running_tasks,
                                queued: queue.len(),
                            });
                        } else {
                            let _ = actor.send_from_us.send(MsgFromImageConversion::DroppedMessage);
                            tracing::debug!("received msg, queue full, dropping");
                        }
                    }
                }
            }
            Some(task_result) = task_result_recv.recv() => {
                let handling_result = actor.on_image_conversion_result(task_result).await;
                if let Err(err) = handling_result {
                    // TODO: do something
                    tracing::error!(?err, "error applying operation");
                }

                running_tasks -= 1;
                if !is_running || (queue.is_empty() && running_tasks == 0) {
                    tracing::debug!("no more messages, idle");
                } else if let Some(msg) = queue.pop_front() {
                    tracing::debug!("dequeuing message");
                    actor.process_message(msg).await;
                    running_tasks += 1;
                }
                let _ = actor.send_from_us.send(MsgFromImageConversion::ActivityChange {
                    running: running_tasks,
                    queued: queue.len(),
                });
            }
        }
    }
    // while let Some(msg) = actor.recv.recv().await {
    //     match msg {
    //         ImageConversionMessage::ConvertImage(convert_image) => {
    //             let res = handle_message(&mut actor, &convert_image).await;
    //             if let Err(report) = res {
    //                 let _ = actor
    //                     .send_result
    //                     .send(ImageConversionResult::ConversionError {
    //                         convert_image,
    //                         report: report.wrap_err("Error running image conversion task"),
    //                     })
    //                     .await;
    //             }
    //         }
    //     }
    // }
}

impl ImageConversionActor {
    #[tracing::instrument(skip(self))]
    async fn process_message(&self, msg: DoTaskMsg) {
        match msg {
            DoTaskMsg::ConvertImage(convert_image) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                let result_send = self.task_result_send.clone();
                tokio::task::spawn(async move {
                    let result =
                        perform_side_effects_convert_image(&convert_image, db_pool, &storage).await;
                    let task_result = result.map(|result| ImageConversionResult {
                        result,
                        convert_image,
                    });
                    result_send.send(task_result).expect("TODO this error must be handled, since the work won't be written to db which is relevant");
                });
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn on_image_conversion_result(&self, task_result: TaskResult) -> Result<()> {
        match task_result {
            Ok(ImageConversionResult {
                result,
                convert_image,
            }) => {
                let mut conn = self.db_pool.get().await?;
                apply_convert_image(&mut conn, &convert_image, result).await?;
            }
            Err(err) => {
                tracing::warn!(?err, "error converting image");
            }
        }
        Ok(())
    }
}
