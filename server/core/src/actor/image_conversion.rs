use eyre::{Report, Result};
use tokio::sync::mpsc;
use tracing::instrument;

use crate::{
    catalog::operation::convert_image::{
        apply_convert_image, perform_side_effects_convert_image, ConvertImage,
    },
    config,
    core::storage::Storage,
    model::repository::db::DbPool,
};

#[derive(Debug, Clone)]
pub enum ImageConversionMessage {
    ConvertImage(ConvertImage),
}

#[derive(Debug)]
pub enum ImageConversionResult {
    ConversionError {
        convert_image: ConvertImage,
        report: Report,
    },
}

pub struct ImageConversionActorHandle {
    pub send: mpsc::Sender<ImageConversionMessage>,
    pub recv_result: mpsc::Receiver<ImageConversionResult>,
}

impl ImageConversionActorHandle {
    pub fn new(db_pool: DbPool, storage: Storage, config: config::Config) -> Self {
        let (send, recv) = mpsc::channel(10000);
        let (send_result, recv_result) = mpsc::channel(1000);
        let actor = ImageConversionActor {
            db_pool,
            storage,
            config,
            recv,
            send_result,
        };
        tokio::spawn(run_image_conversion_actor(actor));
        Self { send, recv_result }
    }
}

struct ImageConversionActor {
    pub db_pool: DbPool,
    pub storage: Storage,
    pub config: config::Config,
    pub recv: mpsc::Receiver<ImageConversionMessage>,
    pub send_result: mpsc::Sender<ImageConversionResult>,
}

async fn run_image_conversion_actor(mut actor: ImageConversionActor) {
    while let Some(msg) = actor.recv.recv().await {
        match msg {
            ImageConversionMessage::ConvertImage(convert_image) => {
                let res = handle_message(&mut actor, &convert_image).await;
                if let Err(report) = res {
                    let _ = actor
                        .send_result
                        .send(ImageConversionResult::ConversionError {
                            convert_image,
                            report: report.wrap_err("Error running image conversion task"),
                        })
                        .await;
                }
            }
        }
    }
}

async fn handle_message(
    actor: &mut ImageConversionActor,
    convert_image: &ConvertImage,
) -> Result<()> {
    let size =
        perform_side_effects_convert_image(&convert_image, actor.db_pool.clone(), &actor.storage)
            .await?;
    let mut conn = actor.db_pool.get().await?;
    let apply_result = apply_convert_image(&mut conn, &convert_image, size).await?;
    Ok(())
}
