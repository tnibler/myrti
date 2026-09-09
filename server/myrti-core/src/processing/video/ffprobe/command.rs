use std::process::Stdio;

use camino::Utf8Path as Path;
use eyre::{Context, Result, eyre};
use itertools::Itertools;
use serde::Deserialize;
use tokio::process::Command;
use tracing::{instrument, warn};

use super::{AudioStream, FFProbeStreams, VideoStream};

pub fn ffprobe_get_streams_from_json(json: &[u8]) -> Result<FFProbeStreams> {
    let parsed_streams = parse_ffprobe_output(json)?;
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
    })
}

#[instrument(err, level = "debug")]
pub async fn ffprobe_get_streams(
    path: &Path,
    (ffprobe_path, args): (&Path, &[String]),
) -> Result<(Vec<u8>, FFProbeStreams)> {
    let ffprobe_result = Command::new(ffprobe_path)
        .args(args)
        .args(["-v", "error", "-show_streams", "-of", "json=compact=1"])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("failed to call ffprobe")?
        .wait_with_output()
        .await
        .wrap_err("ffprobe error")?;
    let parsed_streams = ffprobe_get_streams_from_json(&ffprobe_result.stdout)
        .with_context(|| format!("error parsing ffprobe streams for {}", path))?;
    Ok((ffprobe_result.stdout, parsed_streams))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum StreamType {
    Video(VideoStream),
    Audio(AudioStream),
}

#[allow(dead_code)]
fn parse_ffprobe_output(json: &[u8]) -> Result<Vec<StreamType>> {
    #[derive(Debug, Clone, Deserialize)]
    #[serde(tag = "side_data_type")]
    enum FFProbeSideData {
        #[serde(rename = "Display Matrix")]
        DisplayMatrix { rotation: Option<i32> },
        #[serde(rename = "Frame Cropping")]
        FrameCropping {
            crop_top: Option<i32>,
            crop_bottom: Option<i32>,
            crop_left: Option<i32>,
            crop_right: Option<i32>,
        },
        #[serde(other)]
        Other,
    }
    #[derive(Debug, Clone, Deserialize)]
    struct FFProbeVideoStream {
        pub codec_name: String,
        /// Seconds. Quoted decimal number. May not be present if no stream metadata exists
        pub duration: Option<String>,
        pub width: i32,
        pub height: i32,
        pub bit_rate: Option<String>,
        pub side_data_list: Option<Vec<FFProbeSideData>>,
        pub avg_frame_rate: String,
    }
    #[derive(Debug, Clone, Deserialize)]
    struct FFProbeAudioStream {
        pub codec_name: String,
        pub sample_rate: String,
        pub bit_rate: Option<String>,
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

    let parsed: FFProbeOutput = serde_json::from_slice(json).wrap_err_with(|| {
        format!(
            "could not parse ffprobe output:\n{}",
            String::from_utf8_lossy(json)
        )
    })?;
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
                    .map(|bit_rate| {
                        bit_rate
                            .parse()
                            .wrap_err("could not parse bit_rate ffprobe output")
                    })
                    .transpose()?,
                rotation: match video.side_data_list {
                    Some(side_datas) => side_datas.iter().find_map(|sd| match sd {
                        FFProbeSideData::DisplayMatrix { rotation } => *rotation,
                        _ => None,
                    }),
                    _ => None,
                },
                duration_ms: match video.duration.as_deref().map(str::parse::<f32>) {
                    None => None,
                    Some(Ok(n)) => Some((n * 1000.0).round() as i64),
                    Some(Err(err)) => {
                        // unsure if this can happen, let's see if it ever does
                        debug_assert!(false, "ffprobe: stream duration present but fails to parse");
                        tracing::warn!(
                            video.duration,
                            ?err,
                            "ffprobe: stream duration present but fails to parse"
                        );
                        None
                    }
                },
                avg_frame_rate: video
                    .avg_frame_rate
                    .split_once("/")
                    .and_then(|(num, denom)| {
                        let num = num.parse::<i32>();
                        let denom = denom.parse::<i32>();
                        match (num, denom) {
                            (Ok(n), Ok(d)) => Some((n, d)),
                            (num, denom) => {
                                tracing::warn!(?num, ?denom, "ffprobe: failed to parse framerate");
                                None
                            }
                        }
                    }),
            })),
            FFProbeStreamType::Audio(audio) => Ok(StreamType::Audio(AudioStream {
                codec_name: audio.codec_name,
                sample_rate: audio
                    .sample_rate
                    .parse()
                    .wrap_err("could not parse sample_rate in ffprobe output")?,
                bitrate: audio
                    .bit_rate
                    .map(|bit_rate| {
                        bit_rate
                            .parse()
                            .wrap_err("could not parse bit_rate ffprobe output")
                    })
                    .transpose()?,
                channels: audio.channels,
            })),
            _ => unreachable!("Other case is filtered out"),
        })
        .collect();
    streams
}

/// Max interval (frame count, seconds) between any 2 I-Frames in video stream, or None if there aren't 2 I-Frames.
#[tracing::instrument(level = "trace")]
pub async fn ffprobe_get_max_iframe_interval(
    path: &Path,
    (ffprobe_path, args): (&Path, &[String]),
) -> Result<Option<f64>> {
    let ffprobe_result = Command::new(ffprobe_path)
        .args(args)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-read_intervals",
            "%+20", // read/decode 20 seconds of the stream
            "-skip_frame",
            "nointra", // only decode and output I-frames (much faster)
            "-show_entries",
            "frame=pts_time,pict_type",
            "-of",
            "csv=print_section=0",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("failed to call ffprobe")?
        .wait_with_output()
        .await
        .wrap_err("ffprobe error")?;
    let mut timestamps: Vec<f64> = String::from_utf8(ffprobe_result.stdout)?
        .lines()
        .filter(|line| !line.is_empty()) // timecode or side data entries will produce blank lines
        .map(|line| {
            let (pts, pict_type) = line
                .trim_end_matches(',')
                .split_once(",")
                .ok_or_else(|| eyre!("Unexpected line format in ffprobe output: '{}'", line))?;
            let timestamp = pts.parse().with_context(|| {
                format!(
                    "Error parsing frame timestamps from ffprobe output: '{}'",
                    line
                )
            })?;
            Ok::<_, eyre::Report>((timestamp, pict_type))
        })
        .filter_map_ok(|(timestamp, pict_type)| {
            if pict_type == "I" {
                Some(timestamp)
            } else {
                None
            }
        })
        .try_collect()?;
    // skipping non-I-frames can lead to packets being decoded out of order (RX-100 Mk VII for instance)
    timestamps.sort_unstable_by(|a, b| a.total_cmp(b));
    let max_interval = timestamps
        .iter()
        .tuple_windows()
        .map(|(t_a, t_b)| t_b - t_a)
        .max_by(|delta_t1, delta_t2| delta_t1.total_cmp(delta_t2));
    Ok(max_interval)
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
            bitrate: Some(28034318),
            rotation: Some(-90),
            duration_ms: Some(26285),
            avg_frame_rate: Some((15770000, 262847)),
        }),
        StreamType::Audio(AudioStream {
            codec_name: "aac".into(),
            sample_rate: 48000,
            bitrate: Some(256017),
            channels: 2,
        }),
    ]
    .into_iter()
    .collect();
    let parsed_video_audio: HashSet<_> =
        assert_ok!(parse_ffprobe_output(output_video_audio.as_bytes()))
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
        bitrate: Some(11841634),
        rotation: None,
        duration_ms: Some(30080),
        avg_frame_rate: Some((25, 1)),
    })]
    .into_iter()
    .collect();
    let parsed_video_only: HashSet<_> =
        assert_ok!(parse_ffprobe_output(output_video_only.as_bytes()))
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
        assert_ok!(parse_ffprobe_output(output_video_and_unknown.as_bytes()))
            .into_iter()
            .collect();
    assert_eq!(parsed_video_and_unknown, expected_video_only);
}

#[test]
fn ffprobe_parse_iphone_motion_photo_mov() {
    use claims::assert_ok;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    let output = r#"
{
"streams": [
        {
            "index": 0,
            "codec_name": "hevc",
            "codec_long_name": "H.265 / HEVC (High Efficiency Video Coding)",
            "profile": "Main",
            "codec_type": "video",
            "codec_tag_string": "hvc1",
            "codec_tag": "0x31637668",
            "width": 1920,
            "height": 1440,
            "coded_width": 1920,
            "coded_height": 1440,
            "has_b_frames": 2,
            "pix_fmt": "yuvj420p",
            "level": 150,
            "color_range": "pc",
            "color_space": "smpte170m",
            "color_transfer": "bt709",
            "color_primaries": "smpte432",
            "chroma_location": "left",
            "view_ids_available": "",
            "view_pos_available": "",
            "id": "0x1",
            "r_frame_rate": "240/1",
            "avg_frame_rate": "11600/401",
            "time_base": "1/600",
            "start_pts": 0,
            "start_time": "0.000000",
            "duration_ts": 1203,
            "duration": "2.005000",
            "bit_rate": "14850437",
            "nb_frames": "58",
            "extradata_size": 104,
            "disposition": {
                "default": 1,
                "dub": 0,
                "original": 0,
                "comment": 0,
                "lyrics": 0,
                "karaoke": 0,
                "forced": 0,
                "hearing_impaired": 0,
                "visual_impaired": 0,
                "clean_effects": 0,
                "attached_pic": 0,
                "timed_thumbnails": 0,
                "non_diegetic": 0,
                "captions": 0,
                "descriptions": 0,
                "metadata": 0,
                "dependent": 0,
                "still_image": 0,
                "multilayer": 0
            },
            "tags": {
                "creation_time": "2024-09-06T16:08:07.000000Z",
                "language": "und",
                "handler_name": "Core Media Video",
                "vendor_id": "[0][0][0][0]",
                "encoder": "HEVC"
            },
            "side_data_list": [
                {
                    "side_data_type": "Frame Cropping",
                    "crop_top": 66,
                    "crop_bottom": 66,
                    "crop_left": 88,
                    "crop_right": 88
                },
                {
                    "side_data_type": "Display Matrix",
                    "displaymatrix": "\n00000000:            0       65536           0\n00000001:       -65536           0           0\n00000002:     94371840           0  1073741824\n",
                    "rotation": -90
                }
            ]
        },
        {
            "index": 1,
            "codec_name": "pcm_s16le",
            "codec_long_name": "PCM signed 16-bit little-endian",
            "codec_type": "audio",
            "codec_tag_string": "lpcm",
            "codec_tag": "0x6d63706c",
            "sample_fmt": "s16",
            "sample_rate": "44100",
            "channels": 1,
            "bits_per_sample": 16,
            "initial_padding": 0,
            "id": "0x2",
            "r_frame_rate": "0/0",
            "avg_frame_rate": "0/0",
            "time_base": "1/44100",
            "start_pts": 0,
            "start_time": "0.000000",
            "duration_ts": 88421,
            "duration": "2.005011",
            "bit_rate": "705600",
            "nb_frames": "88445",
            "disposition": {
                "default": 1,
                "dub": 0,
                "original": 0,
                "comment": 0,
                "lyrics": 0,
                "karaoke": 0,
                "forced": 0,
                "hearing_impaired": 0,
                "visual_impaired": 0,
                "clean_effects": 0,
                "attached_pic": 0,
                "timed_thumbnails": 0,
                "non_diegetic": 0,
                "captions": 0,
                "descriptions": 0,
                "metadata": 0,
                "dependent": 0,
                "still_image": 0,
                "multilayer": 0
            },
            "tags": {
                "creation_time": "2024-09-06T16:08:07.000000Z",
                "language": "und",
                "handler_name": "Core Media Audio",
                "vendor_id": "[0][0][0][0]"
            }
        },
        {
            "index": 2,
            "codec_type": "data",
            "codec_tag_string": "mebx",
            "codec_tag": "0x7862656d",
            "id": "0x3",
            "r_frame_rate": "0/0",
            "avg_frame_rate": "0/0",
            "time_base": "1/600",
            "start_pts": 0,
            "start_time": "0.000000",
            "duration_ts": 1203,
            "duration": "2.005000",
            "bit_rate": "39",
            "nb_frames": "1",
            "disposition": {
                "default": 1,
                "dub": 0,
                "original": 0,
                "comment": 0,
                "lyrics": 0,
                "karaoke": 0,
                "forced": 0,
                "hearing_impaired": 0,
                "visual_impaired": 0,
                "clean_effects": 0,
                "attached_pic": 0,
                "timed_thumbnails": 0,
                "non_diegetic": 0,
                "captions": 0,
                "descriptions": 0,
                "metadata": 0,
                "dependent": 0,
                "still_image": 0,
                "multilayer": 0
            },
            "tags": {
                "creation_time": "2024-09-06T16:08:07.000000Z",
                "language": "und",
                "handler_name": "Core Media Metadata"
            }
        },
        {
            "index": 3,
            "codec_type": "data",
            "codec_tag_string": "mebx",
            "codec_tag": "0x7862656d",
            "id": "0x4",
            "r_frame_rate": "0/0",
            "avg_frame_rate": "0/0",
            "time_base": "1/600",
            "start_pts": 0,
            "start_time": "0.000000",
            "duration_ts": 1203,
            "duration": "2.005000",
            "bit_rate": "34856",
            "nb_frames": "58",
            "disposition": {
                "default": 1,
                "dub": 0,
                "original": 0,
                "comment": 0,
                "lyrics": 0,
                "karaoke": 0,
                "forced": 0,
                "hearing_impaired": 0,
                "visual_impaired": 0,
                "clean_effects": 0,
                "attached_pic": 0,
                "timed_thumbnails": 0,
                "non_diegetic": 0,
                "captions": 0,
                "descriptions": 0,
                "metadata": 0,
                "dependent": 0,
                "still_image": 0,
                "multilayer": 0
            },
            "tags": {
                "creation_time": "2024-09-06T16:08:07.000000Z",
                "language": "und",
                "handler_name": "Core Media Metadata"
            }
        },
        {
            "index": 4,
            "codec_type": "data",
            "codec_tag_string": "mebx",
            "codec_tag": "0x7862656d",
            "id": "0x5",
            "r_frame_rate": "0/0",
            "avg_frame_rate": "0/0",
            "time_base": "1/600",
            "start_pts": 820,
            "start_time": "1.366667",
            "duration_ts": 1,
            "duration": "0.001667",
            "bit_rate": "504000",
            "nb_frames": "1",
            "disposition": {
                "default": 1,
                "dub": 0,
                "original": 0,
                "comment": 0,
                "lyrics": 0,
                "karaoke": 0,
                "forced": 0,
                "hearing_impaired": 0,
                "visual_impaired": 0,
                "clean_effects": 0,
                "attached_pic": 0,
                "timed_thumbnails": 0,
                "non_diegetic": 0,
                "captions": 0,
                "descriptions": 0,
                "metadata": 0,
                "dependent": 0,
                "still_image": 0,
                "multilayer": 0
            },
            "tags": {
                "creation_time": "2024-09-06T16:08:07.000000Z",
                "language": "und",
                "handler_name": "Core Media Metadata"
            }
        }
    ]
}
"#;
    let expected: HashSet<StreamType> = [
        StreamType::Video(VideoStream {
            codec_name: "hevc".into(),
            width: 1920,
            height: 1440,
            bitrate: Some(14850437),
            rotation: Some(-90),
            duration_ms: Some(2005),
            avg_frame_rate: Some((11600, 401)),
        }),
        StreamType::Audio(AudioStream {
            codec_name: "pcm_s16le".into(),
            sample_rate: 44100,
            bitrate: Some(705600),
            channels: 1,
        }),
    ]
    .into_iter()
    .collect();
    let parsed: HashSet<_> = assert_ok!(parse_ffprobe_output(output.as_bytes()))
        .into_iter()
        .collect();
    assert_eq!(parsed, expected);
}

#[test]
fn ffprobe_parse_mpeg2_mod() {
    use claims::assert_ok;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    let output = r#"
{
"streams": [
        {
            "index": 0,
            "codec_name": "mpeg2video",
            "codec_long_name": "MPEG-2 video",
            "profile": "Main",
            "codec_type": "video",
            "codec_tag_string": "[0][0][0][0]",
            "codec_tag": "0x0000",
            "width": 720,
            "height": 576,
            "coded_width": 0,
            "coded_height": 0,
            "has_b_frames": 1,
            "sample_aspect_ratio": "64:45",
            "display_aspect_ratio": "16:9",
            "pix_fmt": "yuv420p",
            "level": 8,
            "color_range": "tv",
            "color_space": "bt470bg",
            "color_transfer": "bt470bg",
            "color_primaries": "bt470bg",
            "chroma_location": "left",
            "field_order": "tt",
            "id": "0x1e0",
            "r_frame_rate": "25/1",
            "avg_frame_rate": "25/1",
            "time_base": "1/90000",
            "start_pts": 30800,
            "start_time": "0.342222",
            "duration_ts": 17532000,
            "duration": "194.800000",
            "extradata_size": 98,
            "disposition": {
                "default": 0,
                "dub": 0,
                "original": 0,
                "comment": 0,
                "lyrics": 0,
                "karaoke": 0,
                "forced": 0,
                "hearing_impaired": 0,
                "visual_impaired": 0,
                "clean_effects": 0,
                "attached_pic": 0,
                "timed_thumbnails": 0,
                "non_diegetic": 0,
                "captions": 0,
                "descriptions": 0,
                "metadata": 0,
                "dependent": 0,
                "still_image": 0,
                "multilayer": 0
            },
            "side_data_list": [
                {
                    "side_data_type": "CPB properties",
                    "max_bitrate": 9286000,
                    "min_bitrate": 0,
                    "avg_bitrate": 0,
                    "buffer_size": 1835008,
                    "vbv_delay": -1
                }
            ]
        },
        {
            "index": 1,
            "codec_name": "mp2",
            "codec_long_name": "MP2 (MPEG audio layer 2)",
            "codec_type": "audio",
            "codec_tag_string": "[0][0][0][0]",
            "codec_tag": "0x0000",
            "mime_codec_string": "mp4a.40.33",
            "sample_fmt": "s16p",
            "sample_rate": "48000",
            "channels": 2,
            "channel_layout": "stereo",
            "bits_per_sample": 0,
            "initial_padding": 0,
            "id": "0x1c0",
            "r_frame_rate": "0/0",
            "avg_frame_rate": "0/0",
            "time_base": "1/90000",
            "start_pts": 23600,
            "start_time": "0.262222",
            "duration_ts": 17539200,
            "duration": "194.880000",
            "bit_rate": "256000",
            "disposition": {
                "default": 0,
                "dub": 0,
                "original": 0,
                "comment": 0,
                "lyrics": 0,
                "karaoke": 0,
                "forced": 0,
                "hearing_impaired": 0,
                "visual_impaired": 0,
                "clean_effects": 0,
                "attached_pic": 0,
                "timed_thumbnails": 0,
                "non_diegetic": 0,
                "captions": 0,
                "descriptions": 0,
                "metadata": 0,
                "dependent": 0,
                "still_image": 0,
                "multilayer": 0
            }
        }
    ]
}"#;
    let expected: HashSet<StreamType> = [
        StreamType::Video(VideoStream {
            codec_name: "mpeg2video".into(),
            width: 720,
            height: 576,
            bitrate: None,
            rotation: None,
            duration_ms: Some(194800),
            avg_frame_rate: Some((25, 1)),
        }),
        StreamType::Audio(AudioStream {
            codec_name: "mp2".into(),
            sample_rate: 48000,
            bitrate: Some(256000),
            channels: 2,
        }),
    ]
    .into_iter()
    .collect();
    let parsed: HashSet<_> = assert_ok!(parse_ffprobe_output(output.as_bytes()))
        .into_iter()
        .collect();
    assert_eq!(parsed, expected);
}
