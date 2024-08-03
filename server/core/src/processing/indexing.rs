use camino::Utf8Path as Path;
use chrono::{DateTime, Local, Utc};
use color_eyre::eyre::Result;
use eyre::{eyre, Context};

use crate::{
    config, interact,
    model::{repository::db::DbPool, repository::duplicate_asset::NewDuplicateAsset, *},
    processing::{self, hash::hash_file},
};

use super::{
    media_metadata::{figure_out_utc_timestamp, read_media_metadata, TimestampGuess},
    video::{streams::FFProbeStreamsTrait, FFProbe},
};

/// Returns Some(AssetId) if a new, non duplicate asset was indexed and added to the database
#[tracing::instrument(skip(pool, asset_root, bin_paths))]
pub async fn index_file(
    path: &Path,
    asset_root: &AssetRootDir,
    pool: &DbPool,
    bin_paths: Option<&config::BinPaths>,
) -> Result<Option<AssetId>> {
    let path_in_asset_root = path
        .strip_prefix(&asset_root.path)
        .wrap_err("file to index is not in provided asset root")?;
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
    let exiftool_path = bin_paths
        .and_then(|bp| bp.exiftool.as_ref())
        .map(|p| p.as_path());
    let ffprobe_path = bin_paths
        .and_then(|bp| bp.ffprobe.as_ref())
        .map(|p| p.as_path());
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
            // FIXME ffprobe path should come from config
            let (ffprobe_output, streams) = match FFProbe::streams(path, ffprobe_path).await {
                Ok(r) => r,
                Err(err) => {
                    tracing::trace!(%path, %err, "Could not get stream info with ffprobe, ignoring file");
                    return Ok(None);
                }
            };
            let video = streams.video;
            let create_video = CreateAssetVideo {
                video_codec_name: video.codec_name.to_ascii_lowercase(),
                video_bitrate: video.bitrate,
                audio_codec_name: streams
                    .audio
                    .map(|audio| audio.codec_name.to_ascii_lowercase()),
                has_dash: false,
                ffprobe_output: ffprobe_output.into(),
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
        Some(mime) if mime.starts_with("image") => {
            let p = path.to_owned();
            let vips_get_size_result = tokio::task::spawn_blocking(move || {
                processing::image::get_image_size(&p).wrap_err("could not read image size")
            })
            .await?;
            let size = match vips_get_size_result {
                Ok(s) => s,
                Err(_) => {
                    tracing::trace!(%path, "Could not read image size, ignoring file");
                    return Ok(None);
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
            let size = Size {
                width: size.width,
                height: size.height,
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
        if let Some(existing_asset_id) = existing_with_same_hash {
            repository::duplicate_asset::insert_duplicate_asset(
                conn,
                NewDuplicateAsset {
                    existing_asset_id,
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
        file_path: path.strip_prefix(&asset_root.path)?.to_owned(),
        taken_date: timestamp,
        timestamp_info,
        size,
        rotation_correction: None,
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
