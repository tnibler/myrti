use std::path::Path;

use crate::{
    model::*,
    processing,
    repository::{self, pool::DbPool},
};
use chrono::Utc;
use color_eyre::eyre::Result;
use eyre::Context;
use tracing::{debug, error, instrument, Instrument};
use walkdir::WalkDir;

use super::{
    media_metadata::{figure_out_utc_timestamp, read_media_metadata, TimestampGuess},
    video::probe_video,
};

#[instrument(skip(pool))]
pub async fn index_asset_root(asset_root: &AssetRootDir, pool: &DbPool) -> Result<Vec<AssetId>> {
    let mut new_asset_ids: Vec<AssetId> = vec![];
    // FIXME if a datadir is subdir of assetroot it should obviously not be indexed
    for entry in WalkDir::new(asset_root.path.as_path()).follow_links(true) {
        match entry {
            Ok(e) => {
                if e.file_type().is_file() {
                    if let Some(id) = index_file(e.path(), asset_root, pool)
                        .in_current_span()
                        .await?
                    {
                        new_asset_ids.push(id);
                    }
                }
            }
            Err(e) => {
                if let Some(path) = e.path() {
                    error!("Could not index file or directory {}", path.display());
                } else {
                    error!(
                        "Error during indexing of asset root dir {}",
                        asset_root.path.as_path().display()
                    )
                }
            }
        }
    }
    Ok(new_asset_ids)
}

#[instrument(skip(pool))]
async fn index_file(
    path: &Path,
    asset_root: &AssetRootDir,
    pool: &DbPool,
) -> Result<Option<AssetId>> {
    let path_in_asset_root = path
        .strip_prefix(&asset_root.path)
        .wrap_err("file to index is not in provided asset root")?;
    let existing = repository::asset::asset_with_path_exists(pool, path_in_asset_root).await?;
    if existing {
        return Ok(None);
    }
    let metadata = read_media_metadata(path)
        .in_current_span()
        .await
        .wrap_err("could not read file metadata")?;
    let (ty, full): (AssetType, AssetSpe) = match metadata.file.mime_type.as_ref() {
        Some(mime) if mime.starts_with("video") => {
            let probe = probe_video(path)
                .await
                .wrap_err(format!("file has mimetype {}, but ffprobe failed", mime))?;
            let video_info = AssetSpe::Video(Video {
                codec_name: probe.codec_name,
                bitrate: probe.bitrate,
                dash_resource_dir: None,
            });
            (AssetType::Video, video_info)
        }
        Some(mime) if mime.starts_with("image") => {
            let image_info = AssetSpe::Image(Image {});
            (AssetType::Image, image_info)
        }
        None | Some(_) => {
            debug!(path=%path.display(), "Ignoring file");
            return Ok(None);
        }
    };
    let size: Size = match ty {
        AssetType::Image => {
            let p = path.to_owned();
            let s = tokio::task::spawn_blocking(move || {
                processing::image::get_image_size(&p).wrap_err("could not read image size")
            })
            .await??;
            Size {
                width: s.width.into(),
                height: s.height.into(),
            }
        }
        AssetType::Video => {
            let probe = processing::video::probe_video(path)
                .await
                .wrap_err("could not read video info using ffprobe")?;
            let swap = match probe.rotation {
                Some(n) if n % 180 == 0 => false,
                Some(n) if n % 90 == 0 => true,
                _ => false,
            };
            if swap {
                Size {
                    height: probe.width.into(),
                    width: probe.height.into(),
                }
            } else {
                Size {
                    width: probe.width.into(),
                    height: probe.height.into(),
                }
            }
        }
    };
    let timestamp_guess = figure_out_utc_timestamp(&metadata);
    debug!(?timestamp_guess);
    let timestamp = match timestamp_guess {
        TimestampGuess::None => MediaTimestamp::Utc(Utc::now()),
        TimestampGuess::Utc(utc) => MediaTimestamp::Utc(utc),
        TimestampGuess::LocalOnly(local) => MediaTimestamp::LocalFallback(local),
    };
    let asset_base = AssetBase {
        id: AssetId(0),
        ty,
        root_dir_id: asset_root.id,
        file_path: path.strip_prefix(&asset_root.path)?.to_owned(),
        added_at: Utc::now(),
        taken_date: timestamp,
        size,
        rotation_correction: None,
        hash: None,
        thumb_small_square_avif: None,
        thumb_small_square_webp: None,
        thumb_large_orig_avif: None,
        thumb_large_orig_webp: None,
        thumb_small_square_size: None,
        thumb_large_orig_size: None,
    };
    let full_asset = Asset {
        base: asset_base,
        sp: full,
    };
    let id = repository::asset::insert_asset(pool, &full_asset).await?;
    Ok(Some(id))
}
