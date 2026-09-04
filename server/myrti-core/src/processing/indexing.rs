use camino::Utf8Path as Path;
use chrono::{DateTime, Local, Utc};
use eyre::{Context, Result, eyre};

use myrti_data::db::DbPool;
use myrti_data::model::*;
use myrti_data::repository::duplicate_asset::NewDuplicateAsset;
use myrti_data::{interact, repository};

use crate::processing::media_metadata::exiftool;
use crate::processing::video::ffprobe_get_streams;
use crate::{
    config,
    processing::{self, hash::hash_file},
};

use super::media_metadata::{TimestampGuess, figure_out_utc_timestamp, read_media_metadata};

pub async fn try_index_file(
    path: &Path,
    asset_root: &AssetRootDir,
    canon_root_path: &Path,
    pool: &DbPool,
    bin_paths: Option<&config::BinPaths>,
) -> Result<Option<AssetId>> {
    let path_in_asset_root = path.strip_prefix(canon_root_path).wrap_err_with(|| {
        format!(
            "file to index {} is not in provided asset root {}",
            path, canon_root_path
        )
    })?;
    let path_in_asset_root2 = path_in_asset_root.to_owned();
    let asset_root_id = asset_root.id;
    let conn = pool.get().await?;
    let existing = interact!(conn, move |conn| {
        repository::asset::asset_or_duplicate_with_path_exists(
            conn,
            asset_root_id,
            &path_in_asset_root2,
        )
    })
    .await??;
    if existing {
        return Ok(None);
    }
    drop(conn);
    index_file(path, path_in_asset_root, asset_root, pool, bin_paths).await
}

/// Returns Some(AssetId) if a new, non duplicate asset was indexed and added to the database
#[tracing::instrument(skip_all, fields(path), err, level = "trace")]
async fn index_file(
    path: &Path,
    path_in_asset_root: &Path,
    asset_root: &AssetRootDir,
    pool: &DbPool,
    bin_paths: Option<&config::BinPaths>,
) -> Result<Option<AssetId>> {
    let asset_root_id = asset_root.id;
    let exiftool_path = bin_paths.and_then(|bp| bp.exiftool.as_deref());
    let ffprobe_path = bin_paths.and_then(|bp| bp.ffprobe.as_deref());
    let (exiftool_json, metadata) = read_media_metadata(path, exiftool_path)
        .await
        .wrap_err("could not read file metadata")?;
    let file_type = match &metadata.file.file_type {
        Some(ft) => ft.to_ascii_lowercase(),
        None => {
            tracing::trace!(%path, "Ignoring file: No file type in exiftool output");
            return Ok(None);
        }
    };
    // mime type is a very rough guess that is often wrong. If vips can't open it, we say it's not an
    // image, same with ffprobe and video
    let (create_asset_spe, size): (CreateAssetSpe, Size) = match metadata.file.mime_type.as_ref() {
        Some(mime) if mime.starts_with("video") => {
            let (ffprobe_output, streams) = match ffprobe_get_streams(path, ffprobe_path).await {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(%path, %err, "Could not get stream info with ffprobe, ignoring file");
                    return Ok(None);
                }
            };

            let (is_original_streamable, max_iframe_interval) = if file_type == "mp4" {
                let max_iframe_interval = processing::video::ffprobe_get_max_iframe_interval(
                    path,
                    bin_paths.and_then(|p| p.ffprobe.as_deref()),
                )
                .await?;
                if let Some(interval_seconds) = max_iframe_interval {
                    (
                        interval_seconds < 20.0,
                        Some((interval_seconds * 1000.0).round() as i32),
                    )
                } else {
                    (false, None)
                }
            } else {
                (false, None)
            };
            let video = streams.video;
            let create_video = CreateAssetVideo {
                video_codec_name: video.codec_name.to_ascii_lowercase(),
                video_bitrate: video.bitrate,
                video_duration_ms: video.duration_ms,
                audio_codec_name: streams
                    .audio
                    .map(|audio| audio.codec_name.to_ascii_lowercase()),
                ffprobe_output: ffprobe_output.into(),
                max_iframe_interval,
                frame_rate: video.avg_frame_rate,
            };

            let swap = match video.rotation {
                Some(n) if n % 180 == 0 => false,
                Some(n) if n % 90 == 0 => true,
                _ => false,
            };
            let size = if swap {
                Size {
                    height: video.width,
                    width: video.height,
                }
            } else {
                Size {
                    width: video.width,
                    height: video.height,
                }
            };
            (CreateAssetSpe::Video(create_video), size)
        }
        // TODO: Should probably use libmagic or something faster and more accurate at some point
        Some(mime) if mime.starts_with("image") && !mime.eq_ignore_ascii_case("image/vnd.fpx") => {
            let p = path.to_owned();
            let size = if cfg!(test) {
                if let Some(exiftool::Composite {
                    width: Some(width),
                    height: Some(height),
                    ..
                }) = metadata.composite.as_ref()
                {
                    Size {
                        width: *width,
                        height: *height,
                    }
                } else if let exiftool::File {
                    width: Some(width),
                    height: Some(height),
                    ..
                } = &metadata.file
                {
                    Size {
                        width: *width,
                        height: *height,
                    }
                } else if let Some(exiftool::Exif {
                    width: Some(width),
                    height: Some(height),
                    ..
                }) = metadata.exif.as_ref()
                {
                    Size {
                        width: *width,
                        height: *height,
                    }
                } else {
                    tracing::error!("no image dimensions found in exiftool output");
                    panic!("no image dimensions found in exiftool output")
                }
            } else {
                // Size information from EXIF can be different from the actual image size, so read
                // that by actually decoding the image.
                let vips_result = tokio::task::spawn_blocking(move || {
                    processing::image::get_image_size(&p).wrap_err("could not read image size")
                })
                .await?;
                match vips_result {
                    Ok(s) => Size {
                        width: s.width,
                        height: s.height,
                    },
                    Err(_) => {
                        tracing::info!(%path, "Could not read image size, ignoring file");
                        return Ok(None);
                    }
                }
            };
            let format = metadata
                .file
                .file_type
                .as_ref()
                .ok_or(eyre!("no file type in exiftool output"))?
                .to_ascii_lowercase();
            let create_image = CreateAssetImage {
                image_format_name: format,
            };
            (CreateAssetSpe::Image(create_image), size)
        }
        None | Some(_) => {
            tracing::trace!(%path, "Ignoring file with no or unknown MIME type");
            return Ok(None);
        }
    };
    let file = tokio::fs::File::open(&path)
        .await
        .wrap_err("could not open asset file")?
        .try_into_std()
        .unwrap();
    let hash = hash_file(file).await?;
    let conn = pool.get().await?;
    let path_in_asset_root2 = path_in_asset_root.to_owned();
    let is_duplicate = interact!(conn, move |conn| {
        let existing_with_same_hash = repository::asset::get_asset_with_hash(conn, hash)?;
        if let Some(existing_file_id) = existing_with_same_hash {
            repository::duplicate_asset::insert_duplicate_asset(
                conn,
                NewDuplicateAsset {
                    existing_file_id,
                    asset_root_dir_id: asset_root_id,
                    path_in_asset_root: &path_in_asset_root2,
                },
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    })
    .await??;
    if is_duplicate {
        return Ok(None);
    }
    let timestamp_guess = figure_out_utc_timestamp(&metadata);
    let (timestamp, timestamp_info): (DateTime<Utc>, TimestampInfo) = match timestamp_guess {
        TimestampGuess::None => (Utc::now(), TimestampInfo::NoTimestamp),
        TimestampGuess::Utc(utc) => (utc, TimestampInfo::UtcCertain),
        TimestampGuess::WithTimezone(dt) => (
            dt.with_timezone(&Utc),
            TimestampInfo::TzCertain(*dt.offset()),
        ),
        TimestampGuess::Local(dt) => (
            dt.and_utc(),
            TimestampInfo::TzGuessedLocal(*Local::now().offset()),
        ),
    };
    let coordinates =
        metadata
            .composite
            .and_then(|comp| match (comp.gps_latitude, comp.gps_longitude) {
                (Some(lat), Some(lon)) => Some(GpsCoordinates {
                    lat: (lat * 10e8) as i64,
                    lon: (lon * 10e8) as i64,
                }),
                _ => None,
            });
    let create_asset_base = CreateAssetBase {
        root_dir_id: asset_root.id,
        file_type: file_type.clone(),
        file_path: path_in_asset_root.to_owned(),
        taken_date: timestamp,
        timestamp_info,
        size,
        is_hidden: false,
        rotation_correction: Default::default(),
        exiftool_output: exiftool_json,
        hash: Some(hash),
        gps_coordinates: coordinates,
    };
    let create_asset = CreateAsset {
        base: create_asset_base,
        spe: create_asset_spe,
    };
    let id = interact!(conn, move |conn| {
        repository::asset::create_asset(conn, create_asset)
    })
    .await??;
    Ok(Some(id))
}
