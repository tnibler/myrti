use std::path::Path;

use async_trait::async_trait;
use eyre::Result;
use tracing::Instrument;

use super::{command::ffprobe_get_streams, FFProbe};

#[async_trait]
pub trait FFProbeRotationTrait {
    async fn video_rotation(path: &Path) -> Result<Option<i32>>;
}

#[async_trait]
impl FFProbeRotationTrait for FFProbe {
    async fn video_rotation(path: &Path) -> Result<Option<i32>> {
        let streams = ffprobe_get_streams(path).in_current_span().await;
        streams.map(|streams| streams.video.rotation)
    }
}
