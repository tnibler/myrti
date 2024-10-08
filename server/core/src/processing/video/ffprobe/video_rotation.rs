use async_trait::async_trait;
use camino::Utf8Path as Path;
use eyre::Result;

use super::{command::ffprobe_get_streams, FFProbe};

#[async_trait]
pub trait FFProbeRotationTrait {
    async fn video_rotation(path: &Path, ffprobe_bin_path: Option<&Path>) -> Result<Option<i32>>;
}

#[async_trait]
impl FFProbeRotationTrait for FFProbe {
    async fn video_rotation(path: &Path, ffprobe_bin_path: Option<&Path>) -> Result<Option<i32>> {
        let ffprobe_result = ffprobe_get_streams(path, ffprobe_bin_path)
            
            .await;
        ffprobe_result.map(|(_raw_output, streams)| streams.video.rotation)
    }
}
