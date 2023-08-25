use chrono::{DateTime, FixedOffset, NaiveDateTime, ParseResult, TimeZone, Utc};
use eyre::{eyre, Context, Result};
use serde::Deserialize;
use std::{path::Path, process::Stdio};
use tokio::process::Command;
use tracing::{debug, info_span, Instrument};

pub mod exiftool {
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize)]
    pub struct File {
        #[serde(rename = "MIMEType")]
        pub mime_type: Option<String>,
        #[serde(rename = "FileModifyDate")]
        pub file_modify_date: Option<String>,
        #[serde(rename = "FileAccessDate")]
        pub file_access_date: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct QuickTime {
        #[serde(rename = "CreateDate")]
        pub create_date: Option<String>,
        #[serde(rename = "GPSTimeStamp")]
        pub gps_time_stamp: Option<String>,
        #[serde(rename = "GPSDateTime")]
        pub gps_date_time: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Exif {
        #[serde(rename = "CreateTime")]
        pub create_time: Option<String>,
        #[serde(rename = "CreateDate")]
        pub create_date: Option<String>,
        #[serde(rename = "DateTimeOriginal")]
        pub date_time_original: Option<String>,
        #[serde(rename = "OffsetTime")]
        pub offset_time: Option<String>,
        #[serde(rename = "OffsetTimeOriginal")]
        pub offset_time_original: Option<String>,
        #[serde(rename = "GPSTimeStamp")]
        pub gps_time_stamp: Option<String>,
        #[serde(rename = "GPSDateStamp")]
        pub gps_date_stamp: Option<String>,
        #[serde(rename = "GPSImgDirectionRef")]
        pub gps_img_direction_ref: Option<String>,
        #[serde(rename = "GPSImgDirection")]
        pub gps_img_direction: Option<f64>,
        #[serde(rename = "Orientation")]
        pub orientation: Option<i32>,
        #[serde(rename = "Make")]
        pub make: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Composite {
        #[serde(rename = "GPSAltitude")]
        pub gps_altitude: Option<f64>,
        #[serde(rename = "GPSDateTime")]
        pub gps_date_time: Option<String>,
        #[serde(rename = "GPSLatitude")]
        pub gps_latitude: Option<f64>,
        #[serde(rename = "GPSLongitude")]
        pub gps_longitude: Option<f64>,
        #[serde(rename = "SubSecDateTimeOriginal")]
        pub subsec_date_time_original: Option<String>,
        // created from QuickTime tags
        #[serde(rename = "Rotation")]
        pub rotation: Option<i32>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Output {
        #[serde(rename = "File")]
        pub file: File,
        #[serde(rename = "QuickTime")]
        pub quicktime: Option<QuickTime>,
        #[serde(rename = "EXIF")]
        pub exif: Option<Exif>,
        #[serde(rename = "Composite")]
        pub composite: Option<Composite>,
        /// https://exiftool.org/makernote_types.html
        #[serde(rename = "MakerNotes")]
        pub maker_notes: Option<serde_json::Value>,
    }
}

pub async fn read_media_metadata(path: &Path) -> Result<exiftool::Output> {
    let mut command = Command::new("exiftool");
    command
        .args(["-j", "-g", "-n"])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command
        .spawn()
        .wrap_err("failed to call exiftool")?
        .wait_with_output()
        .instrument(info_span!("exiftool"))
        .await
        .wrap_err("exiftool error")?;
    let mut json_out: Vec<exiftool::Output> =
        serde_json::from_slice(&output.stdout).wrap_err("failed to parse exiftool output")?;
    json_out
        .pop()
        .ok_or(eyre!("failed to parse exiftool output"))
}

#[derive(Debug, Clone)]
pub enum TimestampGuess {
    Utc(DateTime<Utc>),
    LocalOnly(NaiveDateTime),
    None,
}

/// Per spec, EXIF and QuickTime timestamps should be written either as UTC
/// or completed with timezone information in OffsetTime.
/// Some/many camera manufacturers don't do that correctly, so this tries to
/// puzzle together a good guess at the correct timezone
pub fn figure_out_utc_timestamp(et: &exiftool::Output) -> TimestampGuess {
    // manufacturer-specific tags in MakerNotes
    if let Some(ref maker_notes) = et.maker_notes {
        if let Some(make) = et.exif.as_ref().map(|exif| exif.make.as_ref()).flatten() {
            match make.to_lowercase().as_str() {
                "samsung" => {
                    let json_val = maker_notes.get("TimeStamp");
                    if let Some(serde_json::Value::String(ts)) = json_val {
                        let parsed = parse_exiftool_subsecond_timestamp_with_offset(ts);
                        if let Ok(timestamp) = parsed {
                            return TimestampGuess::Utc(timestamp.with_timezone(&Utc));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // maybe we're super lucky and there's a timestamp with timezone right there
    if let Some(ref composite) = et.composite {
        if let Some(ref subsec_date_time_original) = composite.subsec_date_time_original {
            let parsed = parse_exiftool_subsecond_timestamp_with_offset(subsec_date_time_original);
            if let Ok(timestamp) = parsed {
                return TimestampGuess::Utc(timestamp.with_timezone(&Utc));
            }
        }
    }
    // try with CreateTime and OffsetTime from EXIF
    if let Some(ref exif) = et.exif {
        if let Some(offset) = exif
            .offset_time
            .as_ref()
            .or(exif.offset_time_original.as_ref())
        {
            if let Some(ref create_date) = exif.create_date {
                let parsed = parse_exiftool_timestamp_with_offset_time(create_date, offset);
                if let Ok(timestamp) = parsed {
                    return TimestampGuess::Utc(timestamp.with_timezone(&Utc));
                }
            }
        }
    }
    // TODO try with gps timestamps
    // QuickTime, which should be UTC
    if let Some(ref quicktime) = et.quicktime {
        if let Some(ref timestamp) = quicktime.create_date {
            let parsed = parse_exiftool_timestamp_no_offset(timestamp);
            if let Ok(timestamp) = parsed {
                return TimestampGuess::Utc(timestamp.and_utc());
            }
        }
    }
    // No choice but to assume utc unless we know otherwise
    if let Some(ref exif) = et.exif {
        if let Some(ref create_time) = exif.create_time {
            let parsed = parse_exiftool_timestamp_no_offset(create_time);
            if let Ok(timestamp) = parsed {
                return TimestampGuess::Utc(timestamp.and_utc());
            }
        }
    }
    if let Some(ref file_modify_date) = et.file.file_modify_date {
        let parsed = parse_exiftool_timestamp_with_offset(file_modify_date);
        if let Ok(timestamp) = parsed {
            return TimestampGuess::Utc(timestamp.with_timezone(&Utc));
        }
    }
    return TimestampGuess::None;
}

fn parse_exiftool_timestamp_with_offset_time(
    datetime: &str,
    offset: &str,
) -> ParseResult<DateTime<FixedOffset>> {
    parse_exiftool_timestamp_with_offset(format!("{}{}", datetime, offset).as_str())
}

fn parse_exiftool_timestamp_with_offset(s: &str) -> ParseResult<DateTime<FixedOffset>> {
    DateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S%z")
}

fn parse_exiftool_subsecond_timestamp_with_offset(s: &str) -> ParseResult<DateTime<FixedOffset>> {
    DateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S%.f%z")
}

fn parse_exiftool_subsecond_timestamp_no_offset(s: &str) -> ParseResult<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S%.f")
}

fn parse_exiftool_timestamp_no_offset(s: &str) -> ParseResult<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S")
}

#[test]
fn parse_exiftool_timestamps() {
    use claims::*;
    let parsed_with_timezone =
        parse_exiftool_subsecond_timestamp_with_offset("2021:10:13 12:38:37.558+01:00");
    assert_ok!(parsed_with_timezone);
    let parsed_with_missing_offset =
        parse_exiftool_subsecond_timestamp_with_offset("2021:10:13 12:38:37.558");
    assert_err!(parsed_with_missing_offset);
    assert_eq!(
        parsed_with_missing_offset.unwrap_err().kind(),
        chrono::format::ParseErrorKind::TooShort
    );

    let parsed_with_separate_offset =
        parse_exiftool_timestamp_with_offset_time("2021:10:13 12:38:37", "+01:00");
    assert_ok!(parsed_with_separate_offset);
}
