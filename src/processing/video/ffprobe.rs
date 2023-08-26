use std::{path::Path, process::Stdio};

use eyre::{eyre, Context, Result};
use serde::Deserialize;
use tokio::process::Command;
use tracing::{debug, instrument};

#[derive(Debug, Clone, Deserialize)]
pub struct VideoProbeResult {
    pub codec_name: String,
    pub duration_seconds: f32,
    pub width: i64,
    pub height: i64,
    pub bitrate: i64,
    pub rotation: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioProbeResult {
    pub codec_name: String,
    pub sample_rate: i64,
    pub bitrate: i64,
    pub channels: i32,
}

#[instrument()]
pub async fn probe_video(path: &Path) -> Result<VideoProbeResult> {
    #[derive(Debug, Clone, Deserialize)]
    struct SideData {
        pub rotation: Option<i32>,
    }
    #[derive(Debug, Clone, Deserialize)]
    struct Stream {
        pub codec_name: String,
        pub duration: String,
        pub width: i32,
        pub height: i32,
        pub bit_rate: String,
        pub side_data_list: Option<Vec<SideData>>,
    }
    #[derive(Debug, Clone, Deserialize)]
    struct FFProbeOutput {
        pub streams: Vec<Stream>,
    }
    let output = Command::new("ffprobe")
        .args(&[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,duration,bit_rate:stream_side_data=rotation",
            "-of",
            "json",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("failed to call ffprobe")?
        .wait_with_output()
        .await
        .wrap_err("ffprobe error")?;
    serde_json::from_slice::<FFProbeOutput>(&output.stdout)
        .wrap_err("error parsing ffprobe output")
        .map(|probe_json| {
            let stream = probe_json
                .streams
                .get(0)
                .cloned()
                .ok_or_else(|| eyre!("error parsing ffprobe output"))?;
            let rotation: Option<i32> = match stream.side_data_list {
                Some(side_datas) => side_datas.get(0).map(|sd| sd.rotation).flatten(),
                _ => None,
            };
            Ok(VideoProbeResult {
                codec_name: stream.codec_name,
                duration_seconds: stream.duration.parse()?,
                width: stream.width as i64,
                height: stream.height as i64,
                bitrate: stream.bit_rate.parse()?,
                rotation,
            })
        })
        .wrap_err("error parsing ffprobe output")?
}

#[instrument]
pub async fn probe_audio(path: &Path) -> Result<AudioProbeResult> {
    #[derive(Debug, Clone, Deserialize)]
    struct Stream {
        pub codec_name: String,
        pub sample_rate: String,
        pub bit_rate: String,
        pub channels: i32,
    }
    #[derive(Debug, Clone, Deserialize)]
    struct FFProbeOutput {
        pub streams: Vec<Stream>,
    }
    let output = Command::new("ffprobe")
        .args(&[
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,channels,sample_rate,bit_rate",
            "-of",
            "json",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("failed to call ffprobe")?
        .wait_with_output()
        .await
        .wrap_err("ffprobe error")?;
    serde_json::from_slice::<FFProbeOutput>(&output.stdout)
        .wrap_err("error parsing ffprobe output")
        .map(|probe_json| {
            let stream = probe_json
                .streams
                .get(0)
                .cloned()
                .ok_or_else(|| eyre!("error parsing ffprobe output"))?;
            Ok(AudioProbeResult {
                codec_name: stream.codec_name,
                sample_rate: stream
                    .sample_rate
                    .parse()
                    .wrap_err("error parsing ffprobe sample_rate")?,
                bitrate: stream
                    .bit_rate
                    .parse()
                    .wrap_err("error parsing ffprobe bit_rate")?,
                channels: stream.channels,
            })
        })
        .wrap_err("error parsing ffprobe output")?
}
