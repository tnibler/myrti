use camino::Utf8Path as Path;
use chrono::{DateTime, Local, Utc};
use color_eyre::eyre::Result;
use eyre::{eyre, Context};
use tracing::{debug, error, instrument, Instrument};
use walkdir::WalkDir;

use crate::{
    model::{repository::duplicate_asset::NewDuplicateAsset, *},
    processing::{self, hash::hash_file},
    repository::{self, pool::DbPool},
};

use super::{
    media_metadata::{figure_out_utc_timestamp, read_media_metadata, TimestampGuess},
    video::{streams::FFProbeStreamsTrait, FFProbe},
};

#[instrument(skip(pool))]
pub async fn index_asset_root(asset_root: &AssetRootDir, pool: &DbPool) -> Result<Vec<AssetId>> {
    let mut new_asset_ids: Vec<AssetId> = vec![];
    // FIXME if a datadir is subdir of assetroot it should obviously not be indexed
    for entry in WalkDir::new(asset_root.path.as_path()).follow_links(true) {
        match entry {
            Ok(e) => {
                if e.file_type().is_file() {
                    let utf8_path = camino::Utf8Path::from_path(e.path());
                    if let Some(path) = utf8_path {
                        if let Some(id) =
                            index_file(path, asset_root, pool).in_current_span().await?
                        {
                            new_asset_ids.push(id);
                        }
                    }
                }
            }
            Err(e) => {
                if let Some(path) = e.path() {
                    error!("Could not index file or directory {}", path.display());
                } else {
                    error!(
                        "Error during indexing of asset root dir {}",
                        &asset_root.path
                    )
                }
            }
        }
    }
    Ok(new_asset_ids)
}

/// Returns Some(AssetId) if a new, non duplicate asset was indexed and added to the database
#[instrument(skip(pool))]
async fn index_file(
    path: &Path,
    asset_root: &AssetRootDir,
    pool: &DbPool,
) -> Result<Option<AssetId>> {
    let path_in_asset_root = path
        .strip_prefix(&asset_root.path)
        .wrap_err("file to index is not in provided asset root")?;
    let existing = repository::asset::asset_or_duplicate_with_path_exists(
        pool,
        asset_root.id,
        path_in_asset_root,
    )
    .await?;
    if existing {
        return Ok(None);
    }
    let metadata = read_media_metadata(path, Some("exiftool"))
        .in_current_span()
        .await
        .wrap_err("could not read file metadata")?;
    let file_type = match &metadata.file.file_type {
        Some(ft) => ft.to_ascii_lowercase(),
        None => {
            debug!(%path, "No file type in exiftool output, ignoring");
            return Ok(None);
        }
    };
    let (ty, full, size): (AssetType, AssetSpe, Size) = match metadata.file.mime_type.as_ref() {
        Some(mime) if mime.starts_with("video") => {
            let streams = FFProbe::streams(path, Some("ffprobe"))
                .await
                .wrap_err("error getting stream info from file")?;
            let video = streams.video;
            let video_info = AssetSpe::Video(Video {
                video_codec_name: video.codec_name.to_ascii_lowercase(),
                video_bitrate: video.bitrate,
                audio_codec_name: streams
                    .audio
                    .map(|audio| audio.codec_name.to_ascii_lowercase()),
                has_dash: false,
            });
            let swap = match video.rotation {
                Some(n) if n % 180 == 0 => false,
                Some(n) if n % 90 == 0 => true,
                _ => false,
            };
            let size = if swap {
                Size {
                    height: video.width.into(),
                    width: video.height.into(),
                }
            } else {
                Size {
                    width: video.width.into(),
                    height: video.height.into(),
                }
            };
            (AssetType::Video, video_info, size)
        }
        Some(mime) if mime.starts_with("image") => {
            let format = metadata
                .file
                .file_type
                .as_ref()
                .ok_or(eyre!("no file type in exiftool output"))?
                .to_ascii_lowercase();
            let image_info = AssetSpe::Image(Image {
                image_format_name: format,
            });
            let p = path.to_owned();
            let s = tokio::task::spawn_blocking(move || {
                processing::image::get_image_size(&p).wrap_err("could not read image size")
            })
            .await??;
            let size = Size {
                width: s.width.into(),
                height: s.height.into(),
            };
            (AssetType::Image, image_info, size)
        }
        None | Some(_) => {
            debug!(%path, "Ignoring file");
            return Ok(None);
        }
    };
    let file = tokio::fs::File::open(&path)
        .await
        .wrap_err("could not open asset file")?
        .try_into_std()
        .unwrap();
    let hash = hash_file(file).await?;
    let existing_with_same_hash = repository::asset::get_asset_with_hash(pool, hash).await?;
    if let Some(existing_asset_id) = existing_with_same_hash {
        repository::duplicate_asset::insert_duplicate_asset(
            pool,
            NewDuplicateAsset {
                existing_asset_id,
                asset_root_dir_id: asset_root.id,
                path_in_asset_root,
            },
        )
        .in_current_span()
        .await?;
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
    let coordinates = metadata
        .composite
        .map(|comp| match (comp.gps_latitude, comp.gps_longitude) {
            (Some(lat), Some(lon)) => Some(GpsCoordinates {
                lat: (lat * 10e8) as i64,
                lon: (lon * 10e8) as i64,
            }),
            _ => None,
        })
        .flatten();
    let create_asset = CreateAsset {
        ty,
        root_dir_id: asset_root.id,
        file_type: file_type.clone(),
        file_path: path.strip_prefix(&asset_root.path)?.to_owned(),
        taken_date: timestamp,
        timestamp_info,
        size,
        rotation_correction: None,
        hash: Some(hash),
        sp: full,
        gps_coordinates: coordinates,
    };
    let id = repository::asset::create_asset(pool, create_asset).await?;
    Ok(Some(id))
}
