use async_trait::async_trait;
use camino::Utf8Path as Path;
use eyre::Result;
use tracing::Instrument;

use super::{command::ffprobe_get_streams, FFProbe, FFProbeStreams};

#[async_trait]
pub trait FFProbeStreamsTrait {
    async fn streams(path: &Path, ffprobe_bin_path: Option<&str>) -> Result<FFProbeStreams>;
}

#[async_trait]
impl FFProbeStreamsTrait for FFProbe {
    async fn streams(path: &Path, ffprobe_bin_path: Option<&str>) -> Result<FFProbeStreams> {
        ffprobe_get_streams(path, ffprobe_bin_path)
            .in_current_span()
            .await
    }
}
