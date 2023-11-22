use std::process::Stdio;

use camino::Utf8Path as Path;
use eyre::{eyre, Context, Result};
use serde::Deserialize;
use tokio::process::Command;
use tracing::{instrument, warn};

use super::{AudioStream, FFProbeStreams, VideoStream};

#[instrument]
pub async fn ffprobe_get_streams(
    path: &Path,
    ffprobe_bin_path: Option<&str>,
) -> Result<FFProbeStreams> {
    let ffprobe_result = Command::new(ffprobe_bin_path.unwrap_or("ffprobe"))
        .args(&["-v", "error", "-show_streams", "-of", "json=compact=1"])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("failed to call ffprobe")?
        .wait_with_output()
        .await
        .wrap_err("ffprobe error")?;
    let parsed_streams = parse_ffprobe_output(&ffprobe_result.stdout)?;
    let mut video_stream: Option<VideoStream> = None;
    let mut audio_stream: Option<AudioStream> = None;
    for stream in parsed_streams {
        match stream {
            StreamType::Video(s) => match video_stream {
                None => {
                    video_stream = Some(s);
                }
                Some(_) => {
                    warn!("multiple video streams in file")
                }
            },
            StreamType::Audio(s) => match audio_stream {
                None => {
                    audio_stream = Some(s);
                }
                Some(_) => {
                    warn!("multiple audio streams in file")
                }
            },
        };
    }
    Ok(FFProbeStreams {
        video: video_stream.ok_or(eyre!("no video stream found in file"))?,
        audio: audio_stream,
        raw_ffprobe_output: ffprobe_result.stdout,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum StreamType {
    Video(VideoStream),
    Audio(AudioStream),
}

#[allow(dead_code)]
fn parse_ffprobe_output(json: &[u8]) -> Result<Vec<StreamType>> {
    #[derive(Debug, Clone, Deserialize)]
    struct FFProbeSideData {
        pub rotation: Option<i32>,
    }
    #[derive(Debug, Clone, Deserialize)]
    struct FFProbeVideoStream {
        pub codec_name: String,
        pub duration: String,
        pub width: i64,
        pub height: i64,
        pub bit_rate: String,
        pub side_data_list: Option<Vec<FFProbeSideData>>,
    }
    #[derive(Debug, Clone, Deserialize)]
    struct FFProbeAudioStream {
        pub codec_name: String,
        pub sample_rate: String,
        pub bit_rate: String,
        pub channels: i32,
    }
    #[derive(Debug, Clone, Deserialize)]
    #[serde(tag = "codec_type")]
    enum FFProbeStreamType {
        #[serde(rename = "video")]
        Video(FFProbeVideoStream),
        #[serde(rename = "audio")]
        Audio(FFProbeAudioStream),
        #[serde(other)]
        Other,
    }
    #[derive(Debug, Clone, Deserialize)]
    struct FFProbeOutput {
        pub streams: Vec<FFProbeStreamType>,
    }

    let parsed: FFProbeOutput =
        serde_json::from_slice(json).wrap_err("could not parse ffprobe output")?;
    let streams: Result<Vec<StreamType>> = parsed
        .streams
        .into_iter()
        .filter_map(|stream| match stream {
            FFProbeStreamType::Other => None,
            s => Some(s),
        })
        .map(|stream| match stream {
            FFProbeStreamType::Video(video) => Ok(StreamType::Video(VideoStream {
                codec_name: video.codec_name,
                width: video.width,
                height: video.height,
                bitrate: video
                    .bit_rate
                    .parse()
                    .wrap_err("could not parse bit_rate ffprobe output")?,
                rotation: match video.side_data_list {
                    Some(side_datas) => side_datas.get(0).map(|sd| sd.rotation).flatten(),
                    _ => None,
                },
            })),
            FFProbeStreamType::Audio(audio) => Ok(StreamType::Audio(AudioStream {
                codec_name: audio.codec_name,
                sample_rate: audio
                    .sample_rate
                    .parse()
                    .wrap_err("could not parse sample_rate in ffprobe output")?,
                bitrate: audio
                    .bit_rate
                    .parse()
                    .wrap_err("could not parse bit_rate in ffprobe output")?,
                channels: audio.channels,
            })),
            _ => unreachable!("Other case is filtered out"),
        })
        .collect();
    streams
}

#[test]
fn ffprobe_output_parsed_correctly() {
    use claims::assert_ok;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;

    let output_video_audio = r#"
{
    "streams": [
        {
            "index": 0,
            "codec_name": "h264",
            "codec_long_name": "H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10",
            "profile": "High",
            "codec_type": "video",
            "codec_tag_string": "avc1",
            "codec_tag": "0x31637661",
            "width": 1920,
            "height": 1080,
            "coded_width": 1920,
            "coded_height": 1080,
            "closed_captions": 0,
            "film_grain": 0,
            "has_b_frames": 0,
            "sample_aspect_ratio": "1:1",
            "display_aspect_ratio": "16:9",
            "pix_fmt": "yuv420p",
            "level": 41,
            "color_range": "tv",
            "color_space": "bt709",
            "color_transfer": "bt709",
            "color_primaries": "bt709",
            "chroma_location": "left",
            "field_order": "progressive",
            "refs": 1,
            "is_avc": "true",
            "nal_length_size": "4",
            "id": "0x1",
            "r_frame_rate": "60/1",
            "avg_frame_rate": "15770000/262847",
            "time_base": "1/90000",
            "start_pts": 0,
            "start_time": "0.000000",
            "duration_ts": 2365623,
            "duration": "26.284700",
            "bit_rate": "28034318",
            "bits_per_raw_sample": "8",
            "nb_frames": "1577",
            "extradata_size": 34,
            "side_data_list": [
                {
                    "side_data_type": "Display Matrix",
                    "displaymatrix": "\n00000000:            0       65536           0\n00000001:       -65536           0           0\n00000002:            0           0  1073741824\n",
                    "rotation": -90
                }
            ]
        },
        {
            "index": 1,
            "codec_name": "aac",
            "codec_long_name": "AAC (Advanced Audio Coding)",
            "profile": "LC",
            "codec_type": "audio",
            "codec_tag_string": "mp4a",
            "codec_tag": "0x6134706d",
            "sample_fmt": "fltp",
            "sample_rate": "48000",
            "channels": 2,
            "channel_layout": "stereo",
            "bits_per_sample": 0,
            "initial_padding": 0,
            "id": "0x2",
            "r_frame_rate": "0/0",
            "avg_frame_rate": "0/0",
            "time_base": "1/48000",
            "start_pts": 0,
            "start_time": "0.000000",
            "duration_ts": 1261568,
            "duration": "26.282667",
            "bit_rate": "256017",
            "nb_frames": "1232",
            "extradata_size": 2
        }
    ]
}
    "#;
    let expected_video_audio: HashSet<StreamType> = [
        StreamType::Video(VideoStream {
            codec_name: "h264".into(),
            width: 1920,
            height: 1080,
            bitrate: 28034318,
            rotation: Some(-90),
        }),
        StreamType::Audio(AudioStream {
            codec_name: "aac".into(),
            sample_rate: 48000,
            bitrate: 256017,
            channels: 2,
        }),
    ]
    .into_iter()
    .collect();
    let parsed_video_audio: HashSet<_> =
        assert_ok!(parse_ffprobe_output(&output_video_audio.as_bytes()))
            .into_iter()
            .collect();
    assert_eq!(parsed_video_audio, expected_video_audio);

    let output_video_only = r#"
{
    "streams": [
        {
            "index": 0,
            "codec_name": "h264",
            "codec_long_name": "H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10",
            "profile": "Constrained Baseline",
            "codec_type": "video",
            "codec_tag_string": "H264",
            "codec_tag": "0x34363248",
            "width": 1280,
            "height": 720,
            "coded_width": 1280,
            "coded_height": 720,
            "closed_captions": 0,
            "film_grain": 0,
            "has_b_frames": 0,
            "sample_aspect_ratio": "1:1",
            "display_aspect_ratio": "16:9",
            "pix_fmt": "yuv420p",
            "level": 41,
            "chroma_location": "left",
            "field_order": "progressive",
            "refs": 1,
            "is_avc": "false",
            "nal_length_size": "0",
            "r_frame_rate": "25/1",
            "avg_frame_rate": "25/1",
            "time_base": "1/25",
            "start_pts": 0,
            "start_time": "0.000000",
            "duration_ts": 752,
            "duration": "30.080000",
            "bit_rate": "11841634",
            "bits_per_raw_sample": "8",
            "nb_frames": "752",
            "extradata_size": 34
        }
    ]
}
    "#;
    let expected_video_only: HashSet<StreamType> = [StreamType::Video(VideoStream {
        codec_name: "h264".into(),
        width: 1280,
        height: 720,
        bitrate: 11841634,
        rotation: None,
    })]
    .into_iter()
    .collect();
    let parsed_video_only: HashSet<_> =
        assert_ok!(parse_ffprobe_output(&output_video_only.as_bytes()))
            .into_iter()
            .collect();
    assert_eq!(parsed_video_only, expected_video_only);

    // make sure we don't choke on unexpected codec_type values
    let output_video_and_unknown = r#"
{
    "streams": [
        {
            "index": 0,
            "codec_name": "h264",
            "codec_long_name": "H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10",
            "profile": "Constrained Baseline",
            "codec_type": "video",
            "codec_tag_string": "H264",
            "codec_tag": "0x34363248",
            "width": 1280,
            "height": 720,
            "coded_width": 1280,
            "coded_height": 720,
            "closed_captions": 0,
            "film_grain": 0,
            "has_b_frames": 0,
            "sample_aspect_ratio": "1:1",
            "display_aspect_ratio": "16:9",
            "pix_fmt": "yuv420p",
            "level": 41,
            "chroma_location": "left",
            "field_order": "progressive",
            "refs": 1,
            "is_avc": "false",
            "nal_length_size": "0",
            "r_frame_rate": "25/1",
            "avg_frame_rate": "25/1",
            "time_base": "1/25",
            "start_pts": 0,
            "start_time": "0.000000",
            "duration_ts": 752,
            "duration": "30.080000",
            "bit_rate": "11841634",
            "bits_per_raw_sample": "8",
            "nb_frames": "752",
            "extradata_size": 34
        },
        {
            "index": 1,
            "codec_type": "couldbeanythingreally"
        }
    ]
}
    "#;
    let parsed_video_and_unknown: HashSet<_> =
        assert_ok!(parse_ffprobe_output(&output_video_and_unknown.as_bytes()))
            .into_iter()
            .collect();
    assert_eq!(parsed_video_and_unknown, expected_video_only);
}
