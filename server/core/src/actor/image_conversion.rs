use eyre::{Report, Result};
use tokio::sync::mpsc;
use tracing::Instrument;

use crate::{
    catalog::operation::convert_image::{
        apply_convert_image, perform_side_effects_convert_image, ConvertImage,
        ImageConversionSideEffectResult,
    },
    core::storage::Storage,
    model::repository::db::DbPool,
};

use super::simple_queue_actor::{
    Actor, ActorOptions, MsgFrom, MsgTaskControl, QueuedActorHandle, TaskId,
};

pub type ImageConversionTaskMsg = ConvertImage;
pub type ImageConversionActorHandle = QueuedActorHandle<ImageConversionTaskMsg>;
pub type MsgFromImageConversion = MsgFrom<ImageConversionTaskResult>;

#[derive(Debug)]
pub enum ImageConversionTaskResult {
    ConversionComplete(ConvertImage),
    ConversionError {
        convert_image: ConvertImage,
        report: Report,
    },
}

pub fn start_image_conversion_actor(
    db_pool: DbPool,
    storage: Storage,
    send_from_us: mpsc::UnboundedSender<MsgFromImageConversion>,
) -> ImageConversionActorHandle {
    let actor = ImageConversionActor { db_pool, storage };
    QueuedActorHandle::new(
        actor,
        send_from_us,
        ActorOptions {
            max_tasks: 8,
            max_queue_size: 1000,
        },
        tracing::info_span!("image_conversion"),
    )
}

impl QueuedActorHandle<ImageConversionTaskMsg> {
    pub fn msg_convert_image(&self, msg: ConvertImage) -> Result<()> {
        self.msg_do_task(msg)
    }
}

struct ImageConversionActor {
    db_pool: DbPool,
    storage: Storage,
}

impl Actor<ImageConversionTaskMsg, ImageConversionTaskResult> for ImageConversionActor {
    async fn run_task(
        &mut self,
        msg: ImageConversionTaskMsg,
        result_send: mpsc::UnboundedSender<(TaskId, ImageConversionTaskResult)>,
        task_id: TaskId,
        _ctl_recv: mpsc::UnboundedReceiver<MsgTaskControl>,
    ) {
        let db_pool = self.db_pool.clone();
        let storage = self.storage.clone();
        async fn apply_result(
            db_pool: DbPool,
            convert_image: &ConvertImage,
            result: ImageConversionSideEffectResult,
        ) -> Result<()> {
            let mut conn = db_pool.get().await?;
            apply_convert_image(&mut conn, convert_image, result).await?;
            Ok(())
        }
        tokio::task::spawn(
            async move {
                let result =
                    perform_side_effects_convert_image(&msg, db_pool.clone(), &storage).await;
                match result {
                    Ok(result) => {
                        let apply_result = apply_result(db_pool, &msg, result).await;
                        match apply_result {
                            Ok(()) => {
                                result_send
                                    .send((
                                        task_id,
                                        ImageConversionTaskResult::ConversionComplete(msg),
                                    ))
                                    .expect("Receiver must be alive");
                            }
                            Err(report) => {
                                result_send
                                    .send((
                                        task_id,
                                        ImageConversionTaskResult::ConversionError {
                                            convert_image: msg,
                                            report,
                                        },
                                    ))
                                    .expect("Receiver must be alive");
                            }
                        }
                    }
                    Err(report) => {
                        result_send
                            .send((
                                task_id,
                                ImageConversionTaskResult::ConversionError {
                                    convert_image: msg,
                                    report,
                                },
                            ))
                            .expect("Receiver must be alive");
                    }
                };
            }
            .in_current_span(),
        );
    }
}
