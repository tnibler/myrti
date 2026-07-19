use std::process::Stdio;

use camino::Utf8Path as Path;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, ParseResult, Utc};
use eyre::{Context, Result, eyre};
use tokio::process::Command;
use tracing::{Instrument, debug_span};

pub mod exiftool {
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize)]
    pub struct File {
        #[serde(rename = "MIMEType")]
        pub mime_type: Option<String>,
        #[serde(rename = "FileType")]
        pub file_type: Option<String>,
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
        // #[serde(rename = "GPSImgDirectionRef")]
        // pub gps_img_direction_ref: Option<String>,
        // #[serde(rename = "GPSImgDirection")]
        // pub gps_img_direction: Option<String>,
        // #[serde(rename = "Orientation")]
        // pub orientation: Option<i32>,
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

#[tracing::instrument]
pub async fn read_media_metadata(
    path: &Path,
    exiftool_bin_path: Option<&Path>,
) -> Result<(Vec<u8>, exiftool::Output)> {
    let mut command = Command::new(exiftool_bin_path.unwrap_or("exiftool".into()));
    command
        .args([
            "-j", // JSON
            "-g", // group headings (File, EXIF, Maker Notes, Copmosite)
            "-n", // no print conversion to human readable
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command
        .spawn()
        .wrap_err("failed to call exiftool")?
        .wait_with_output()
        .instrument(debug_span!("exiftool"))
        .await
        .wrap_err("exiftool error")?;
    let raw_json = output.stdout;
    let parsed: exiftool::Output = serde_json::from_slice::<Vec<exiftool::Output>>(&raw_json)
        .wrap_err("failed to parse exiftool output")?
        .pop() // json is an array with a single element
        .ok_or(eyre!("failed to parse exiftool output"))?;
    Ok((raw_json, parsed))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimestampGuess {
    WithTimezone(DateTime<FixedOffset>),
    Utc(DateTime<Utc>),
    Local(NaiveDateTime),
    None,
}

/// Per spec, EXIF and QuickTime timestamps should be written either as UTC
/// or completed with timezone information in OffsetTime.
/// Some/many camera manufacturers don't do that correctly, so this tries to
/// puzzle together a good guess at the correct timezone
pub fn figure_out_utc_timestamp(et: &exiftool::Output) -> TimestampGuess {
    // manufacturer-specific tags in MakerNotes
    if let Some(maker_notes) = &et.maker_notes
        && let Some(make) = et.exif.as_ref().and_then(|exif| exif.make.as_deref())
    {
        match make.to_lowercase().as_str() {
            "samsung" => {
                let json_val = maker_notes.get("TimeStamp");
                if let Some(serde_json::Value::String(ts)) = json_val {
                    let parsed = parse_exiftool_subsecond_timestamp_with_offset(ts);
                    if let Ok(timestamp) = parsed {
                        return TimestampGuess::WithTimezone(timestamp);
                    }
                }
            }
            _ => {}
        }
    }
    // maybe we're super lucky and there's a timestamp with timezone right there
    if let Some(composite) = &et.composite
        && let Some(subsec_date_time_original) = composite.subsec_date_time_original.as_deref()
    {
        let parsed = parse_exiftool_subsecond_timestamp_with_offset(subsec_date_time_original);
        if let Ok(timestamp) = parsed {
            return TimestampGuess::WithTimezone(timestamp);
        }
    }
    // try with CreateTime and OffsetTime from EXIF
    if let Some(ref exif) = et.exif
        && let Some(offset) = exif
            .offset_time
            .as_deref()
            .or(exif.offset_time_original.as_deref())
        && let Some(ref create_date) = exif.create_date
    {
        let parsed = parse_exiftool_timestamp_with_offset_time(create_date, offset);
        if let Ok(timestamp) = parsed {
            return TimestampGuess::WithTimezone(timestamp);
        }
    }

    // TODO try with gps timestamps

    if let Some(subsec_date_time_original) = &et
        .composite
        .as_ref()
        .and_then(|composite| composite.subsec_date_time_original.as_deref())
    {
        let parsed = parse_exiftool_subsecond_timestamp_no_offset(subsec_date_time_original);
        if let Ok(timestamp) = parsed {
            return TimestampGuess::Local(timestamp);
        }
    }

    if let Some(date_time_original) = &et
        .exif
        .as_ref()
        .and_then(|exif| exif.date_time_original.as_deref())
    {
        let parsed = parse_exiftool_timestamp_no_offset(date_time_original);
        if let Ok(timestamp) = parsed {
            return TimestampGuess::Local(timestamp);
        }
    }

    // QuickTime, which should be UTC <-- source?
    if let Some(quicktime) = &et.quicktime
        && let Some(timestamp) = quicktime.create_date.as_deref()
    {
        let parsed = parse_exiftool_timestamp_no_offset(timestamp);
        if let Ok(timestamp) = parsed {
            return TimestampGuess::Utc(timestamp.and_utc());
        }
    }

    // No choice but to assume utc unless we know otherwise
    if let Some(exif) = &et.exif
        && let Some(create_time) = exif.create_time.as_deref()
    {
        let parsed = parse_exiftool_timestamp_no_offset(create_time);
        if let Ok(timestamp) = parsed {
            return TimestampGuess::Utc(timestamp.and_utc());
        }
    }
    // FIXME is this ever what we want
    if let Some(ref file_modify_date) = et.file.file_modify_date {
        let parsed = parse_exiftool_timestamp_with_offset(file_modify_date);
        if let Ok(timestamp) = parsed {
            return TimestampGuess::WithTimezone(timestamp);
        }
    }
    TimestampGuess::None
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

#[test]
fn exiftool_timestamp_panasonic() {
    let output = r#"
[{
  "SourceFile": "/home/data/library2/P1080723.RW2",
  "ExifTool": {
    "ExifToolVersion": 13.59
  },
  "File": {
    "FileName": "P1080723.RW2",
    "Directory": "/home/data/library2",
    "FileSize": 14871040,
    "FileModifyDate": "2026:07:19 10:06:36+02:00",
    "FileAccessDate": "2026:07:19 11:19:12+02:00",
    "FileInodeChangeDate": "2026:07:19 10:06:36+02:00",
    "FilePermissions": 100755,
    "FileType": "RW2",
    "FileTypeExtension": "RW2",
    "MIMEType": "image/x-panasonic-rw2",
    "ExifByteOrder": "II",
    "ImageWidth": 1920,
    "ImageHeight": 1080,
    "EncodingProcess": 0,
    "BitsPerSample": 8,
    "ColorComponents": 3,
    "YCbCrSubSampling": "2 1"
  },
  "EXIF": {
    "PanasonicRawVersion": "0360",
    "SensorWidth": 4816,
    "SensorHeight": 2600,
    "SensorTopBorder": 8,
    "SensorLeftBorder": 8,
    "SensorBottomBorder": 2592,
    "SensorRightBorder": 4600,
    "SamplesPerPixel": 1,
    "CFAPattern": 4,
    "BitsPerSample": 12,
    "Compression": 34316,
    "LinearityLimitRed": 4095,
    "LinearityLimitGreen": 4095,
    "LinearityLimitBlue": 4095,
    "ISO": 800,
    "HighISOMultiplierRed": 1,
    "HighISOMultiplierGreen": 1,
    "HighISOMultiplierBlue": 1,
    "NoiseReductionParams": "5 100 4 4 4 200 8 8 8 400 16 16 16 800 32 32 32 1600 64 64 64",
    "BlackLevelRed": 128,
    "BlackLevelGreen": 128,
    "BlackLevelBlue": 128,
    "WBRedLevel": 615,
    "WBGreenLevel": 256,
    "WBBlueLevel": 568,
    "RawFormat": 4,
    "Orientation": 1,
    "XResolution": 180,
    "YResolution": 180,
    "ResolutionUnit": 2,
    "Software": "Ver.2.1",
    "ModifyDate": "2016:08:13 13:18:23",
    "YCbCrPositioning": 2,
    "SensitivityType": 1,
    "ComponentsConfiguration": "1 2 3 0",
    "CompressedBitsPerPixel": 2,
    "LightSource": 0,
    "SubSecTime": "010",
    "FlashpixVersion": "0100",
    "ColorSpace": 1,
    "ExifImageWidth": 1920,
    "ExifImageHeight": 1080,
    "InteropIndex": "R98",
    "InteropVersion": "0100",
    "SensingMethod": 2,
    "SceneType": 1,
    "CustomRendered": 0,
    "ExposureMode": 1,
    "WhiteBalance": 0,
    "DigitalZoomRatio": 0,
    "FocalLengthIn35mmFormat": 119,
    "SceneCaptureType": 0,
    "GainControl": 2,
    "Contrast": 0,
    "Saturation": 0,
    "Sharpness": 0,
    "ThumbnailOffset": 35840,
    "ThumbnailLength": 6399,
    "JpgFromRaw": "(Binary data 546816 bytes, use -b option to extract)",
    "CropTop": 0,
    "CropLeft": 0,
    "CropBottom": 0,
    "CropRight": 0,
    "Make": "Panasonic",
    "Model": "DMC-G70",
    "StripOffsets": 4294967295,
    "RowsPerStrip": 2600,
    "StripByteCounts": 0,
    "RawDataOffset": 551424,
    "Gamma": 1.17578125,
    "ExposureTime": 0.0125,
    "FNumber": 5.6,
    "ExposureProgram": 1,
    "ExifVersion": "0230",
    "DateTimeOriginal": "2016:08:13 13:18:23",
    "CreateDate": "2016:08:13 13:18:23",
    "ExposureCompensation": 0,
    "MaxApertureValue": 5.12453775108586,
    "MeteringMode": 5,
    "Flash": 16,
    "FocalLength": 55,
    "SubSecTimeOriginal": "010",
    "SubSecTimeDigitized": "010",
    "FileSource": 3,
    "ThumbnailImage": "(Binary data 6399 bytes, use -b option to extract)"
  },
  "PanasonicRaw": {
    "NumWBEntries": 7,
    "WBType1": 9,
    "WB_RGBLevels1": "601 256 409",
    "WBType2": 10,
    "WB_RGBLevels2": "644 256 380",
    "WBType3": 11,
    "WB_RGBLevels3": "688 256 361",
    "WBType4": 3,
    "WB_RGBLevels4": "419 256 597",
    "WBType5": 4,
    "WB_RGBLevels5": "606 256 392",
    "WBType6": 20,
    "WB_RGBLevels6": "567 256 410",
    "WBType7": 24,
    "WB_RGBLevels7": "419 256 597",
    "DistortionParam02": 0,
    "DistortionParam04": 0,
    "DistortionScale": 1,
    "DistortionCorrection": 0,
    "DistortionParam08": 0,
    "DistortionParam09": 0,
    "DistortionParam11": 0,
    "FocusStepNear": 14,
    "FocusStepCount": 65535,
    "FlashFired": 0,
    "LensAttached": 1,
    "LensTypeMake": 2,
    "LensTypeModel": "19 10",
    "FocalLengthIn35mmFormat": 110,
    "ApertureValue": 5.55816774268302,
    "ShutterSpeedValue": 0.0123792051972851,
    "SensitivityValue": 3,
    "FacesDetected": 0,
    "WB_CFA0_LevelDaylight": 1642,
    "WB_CFA1_LevelDaylight": 1024,
    "WB_CFA2_LevelDaylight": 1024,
    "WB_CFA3_LevelDaylight": 2271,
    "WhiteBalanceSet": 0,
    "WB_RedLevelAuto": 2461,
    "WB_BlueLevelAuto": 2273,
    "Orientation": 1,
    "WhiteBalanceDetected": 0
  },
  "MakerNotes": {
    "ImageQuality": 7,
    "FirmwareVersion": "0 2 1 0",
    "WhiteBalance": 1,
    "FocusMode": 1,
    "AFAreaMode": "0 49",
    "ImageStabilization": 2,
    "MacroMode": 2,
    "ShootingMode": 11,
    "Audio": 2,
    "DataDump": "(Binary data 24584 bytes, use -b option to extract)",
    "FlashBias": -1,
    "InternalSerialNumber": "XEL1605200162",
    "PanasonicExifVersion": "0411",
    "VideoFrameRate": 0,
    "ColorEffect": 1,
    "TimeSincePowerOn": 492.01,
    "BurstMode": 0,
    "SequenceNumber": 0,
    "ContrastMode": 1,
    "NoiseReduction": 0,
    "SelfTimer": 1,
    "Rotation": 1,
    "AFAssistLamp": 2,
    "ColorMode": 0,
    "OpticalZoomMode": 1,
    "ConversionLens": 1,
    "TravelDay": 65535,
    "BatteryLevel": 2,
    "Contrast": 0,
    "WorldTimeLocation": 1,
    "ProgramISO": 800,
    "AdvancedSceneType": 1,
    "FacesDetected": 0,
    "Saturation": 0,
    "Sharpness": 0,
    "JPEGQuality": 255,
    "ColorTempKelvin": 3300,
    "BracketSettings": 0,
    "WBShiftAB": 0,
    "WBShiftGM": 0,
    "FlashCurtain": 0,
    "LongExposureNoiseReduction": 2,
    "PanasonicImageWidth": 4592,
    "PanasonicImageHeight": 2584,
    "AFPointPosition": "0.26171875 0.5",
    "NumFacePositions": 0,
    "LensType": "LUMIX G VARIO 14-140/F3.5-5.6",
    "LensSerialNumber": "06EIB13X065N",
    "AccessoryType": "NO-ACCESSORY",
    "AccessorySerialNumber": "0000000",
    "LensFirmwareVersion": "0 1 1 0",
    "FacesRecognized": 0,
    "Title": "",
    "BabyName": "",
    "Location": "",
    "Country": "",
    "State": "",
    "City": "",
    "Landmark": "",
    "IntelligentResolution": 0,
    "MergedImages": 0,
    "BurstSpeed": 0,
    "IntelligentD-Range": 0,
    "ClearRetouch": 0,
    "City2": "",
    "PhotoStyle": 1,
    "ShadingCompensation": 0,
    "WBShiftIntelligentAuto": 0,
    "AccelerometerZ": 270,
    "AccelerometerX": 3,
    "AccelerometerY": -20,
    "CameraOrientation": 0,
    "RollAngle": 0.5,
    "PitchAngle": 0.3,
    "WBShiftCreativeControl": 0,
    "SweepPanoramaDirection": 0,
    "SweepPanoramaFieldOfView": 0,
    "TimerRecording": 0,
    "InternalNDFilter": 0,
    "HDR": 0,
    "ShutterType": 0,
    "FilterEffect": "0 0",
    "ClearRetouchValue": "undef",
    "OutputLUT": "(Binary data 864 bytes, use -b option to extract)",
    "TouchAE": 0,
    "MonochromeFilterEffect": 0,
    "HighlightShadow": "0 0",
    "TimeStamp": "2016:08:13 12:18:23",
    "VideoBurstResolution": 1,
    "MultiExposure": 1,
    "RedEyeRemoval": 0,
    "VideoBurstMode": 1,
    "DiffractionCorrection": 1,
    "TimeLapseShotNumber": 0,
    "MakerNoteVersion": "0151",
    "SceneMode": 0,
    "HighlightWarning": 1,
    "DarkFocusEnvironment": 1,
    "WBRedLevel": 2461,
    "WBGreenLevel": 1024,
    "WBBlueLevel": 2273,
    "TextStamp": 1,
    "BabyAge": "9999:99:99 00:00:00"
  },
  "PrintIM": {
    "PrintIMVersion": "0250"
  },
  "Composite": {
    "Aperture": 5.6,
    "BlueBalance": 2.21875,
    "RedBalance": 2.40234375,
    "ShutterSpeed": 0.0125,
    "SubSecCreateDate": "2016:08:13 13:18:23.010",
    "SubSecDateTimeOriginal": "2016:08:13 13:18:23.010",
    "SubSecModifyDate": "2016:08:13 13:18:23.010",
    "LensType": "2 19 10",
    "AdvancedSceneMode": "DMC-G70 0 1",
    "ImageHeight": 2584,
    "ImageWidth": 4592,
    "ImageSize": "4592 2584",
    "LensID": "2 19 10",
    "LightValue": 8.29278174922785,
    "Megapixels": 11.865728,
    "ScaleFactor35efl": 2,
    "CircleOfConfusion": 0.0150231303144333,
    "FOV": 18.5866328982514,
    "FocalLength35efl": 110,
    "HyperfocalDistance": 35.9564591481711
  }
}]
    "#;
    let exif: exiftool::Output = serde_json::from_slice::<Vec<exiftool::Output>>(output.as_bytes())
        .expect("failed to parse exiftool output")
        .pop() // json is an array with a single element
        .expect("failed to parse exiftool output");
    assert_eq!(
        exif.file.mime_type.as_deref(),
        Some("image/x-panasonic-rw2")
    );
    assert!(exif.quicktime.is_none());
    assert!(exif.composite.as_ref().unwrap().rotation.is_none());
    assert!(exif.composite.as_ref().unwrap().gps_latitude.is_none());
    assert!(exif.composite.as_ref().unwrap().gps_date_time.is_none());
    assert_eq!(
        exif.composite
            .as_ref()
            .unwrap()
            .subsec_date_time_original
            .as_deref(),
        Some("2016:08:13 13:18:23.010")
    );
    assert_eq!(
        exif.exif.as_ref().unwrap().date_time_original.as_deref(),
        Some("2016:08:13 13:18:23")
    );
    assert_eq!(
        exif.exif.as_ref().unwrap().create_date.as_deref(),
        Some("2016:08:13 13:18:23")
    );
    assert!(exif.exif.as_ref().unwrap().create_time.is_none());

    let expected = TimestampGuess::Local(NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2016, 8, 13).unwrap(),
        NaiveTime::from_hms_milli_opt(13, 18, 23, 10).unwrap(),
    ));
    assert_eq!(expected, figure_out_utc_timestamp(&exif));
}
