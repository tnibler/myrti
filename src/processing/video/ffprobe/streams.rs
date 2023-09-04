use std::path::Path;

use async_trait::async_trait;
use eyre::Result;
use tracing::Instrument;

use super::{command::ffprobe_get_streams, FFProbe, FFProbeStreams};

#[async_trait]
pub trait FFProbeStreamsTrait {
    async fn streams(path: &Path) -> Result<FFProbeStreams>;
}

#[async_trait]
impl FFProbeStreamsTrait for FFProbe {
    async fn streams(path: &Path) -> Result<FFProbeStreams> {
        ffprobe_get_streams(path).in_current_span().await
    }
}
