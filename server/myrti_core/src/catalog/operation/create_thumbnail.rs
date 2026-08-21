use camino::Utf8PathBuf as PathBuf;
use eyre::{Context, Report, Result};
use futures::{TryStreamExt, stream::FuturesUnordered};

use crate::{
    core::storage::{CommandOutputFile, Storage, StorageCommandOutput, StorageProvider},
    interact,
    model::{
        Asset, AssetFile, AssetId, AssetSpe, AssetThumbnail, AssetThumbnailId, AssetType, FileId,
        Size, ThumbnailFormat, ThumbnailType,
        repository::{
            self,
            db::{DbPool, PooledDbConn},
        },
    },
    processing::{
        self,
        commands::GenerateThumbnail,
        image::thumbnail::{GenerateThumbnailTrait, ThumbnailParams, ThumbnailResult},
        process_control::ProcessControlReceiver,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAssetThumbnail {
    pub file_id: FileId,
    pub thumbnails: Vec<ThumbnailToCreate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAssetThumbhash {
    pub file_id: FileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailToCreate {
    pub ty: ThumbnailType,
    pub formats: Vec<ThumbnailFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateThumbnailWithPaths {
    pub file_id: FileId,
    pub thumbnails: Vec<ThumbnailToCreateWithPaths>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailToCreateWithPaths {
    pub ty: ThumbnailType,
    pub file_keys: Vec<(ThumbnailFormat, String)>,
}

pub async fn apply_create_thumbnail(
    conn: &mut PooledDbConn,
    file_id: FileId,
    result: ThumbnailSideEffectSuccess,
) -> Result<()> {
    interact!(conn, move |conn| {
        repository::asset::insert_asset_thumbnail(
            conn,
            AssetThumbnail {
                id: AssetThumbnailId(0),
                file_id,
                ty: result.ty,
                size: result.actual_size,
                format: result.format,
            },
        )?;
        Ok(())
    })
    .await??;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ThumbnailSideEffectSuccess {
    pub ty: ThumbnailType,
    pub format: ThumbnailFormat,
    pub actual_size: Size,
}

#[derive(Debug)]
pub struct ThumbnailSideEffectResult {
    pub file_id: FileId,
    pub succeeded: Vec<ThumbnailSideEffectSuccess>,
    pub failed: Vec<(ThumbnailToCreateWithPaths, Report)>,
}

pub async fn perform_side_effects_create_thumbnail(
    storage: &Storage,
    pool: DbPool,
    op: CreateThumbnailWithPaths,
    control_recv: &mut ProcessControlReceiver,
) -> Result<ThumbnailSideEffectResult> {
    let mut result = ThumbnailSideEffectResult {
        file_id: op.file_id,
        succeeded: Vec::default(),
        failed: Vec::default(),
    };
    if op.thumbnails.is_empty() {
        return Ok(result);
    }
    let conn = pool.get().await?;
    let (in_path, file) = interact!(conn, move |conn| {
        let in_path = repository::asset::get_asset_path_on_disk(conn, op.file_id)?.path_on_disk();
        let file = repository::asset::get_asset_file(conn, op.file_id)?;
        Ok::<_, eyre::Report>((in_path, file))
    })
    .await??;
    // TODO don't await sequentially. Not super bad because op.thumbnails is small but still
    for thumb in op.thumbnails {
        match create_thumbnail(in_path.clone(), &file, &thumb, storage, control_recv).await {
            Ok(res) => {
                for (format, _file_key) in thumb.file_keys {
                    result.succeeded.push(ThumbnailSideEffectSuccess {
                        ty: thumb.ty,
                        format,
                        actual_size: res.actual_size,
                    });
                }
            }
            Err(err) => {
                result.failed.push((thumb.clone(), err));
            }
        }
    }
    Ok(result)
}

async fn create_thumbnail(
    asset_path: PathBuf,
    file: &AssetFile,
    thumb: &ThumbnailToCreateWithPaths,
    storage: &Storage,
    control_recv: &mut ProcessControlReceiver,
) -> Result<ThumbnailResult> {
    let out_files: Vec<CommandOutputFile> = thumb
        .file_keys
        .iter()
        .map(|(_format, key)| storage.new_command_out_file(key))
        .collect::<FuturesUnordered<_>>()
        .try_collect()
        .await
        .wrap_err("error creating asset thumbnail output files")?;
    let out_dimension = match thumb.ty {
        ThumbnailType::SmallSquare => processing::image::OutDimension::Crop {
            width: 200,
            height: 200,
        },
        ThumbnailType::LargeOrigAspect => processing::image::OutDimension::KeepAspect {
            width: (file.size.width as f32 * (300.0 / file.size.height as f32)).round() as i32,
        },
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let thumbnail_params = ThumbnailParams {
        in_path: asset_path,
        outputs: out_files.iter().collect(),
        out_dimension,
    };
    let res = match file.ty {
        AssetType::Image => GenerateThumbnail::generate_thumbnail(thumbnail_params).await,
        AssetType::Video => {
            GenerateThumbnail::generate_video_thumbnail(thumbnail_params, control_recv).await
        }
    };
    tx.send(res).unwrap();
    let result = rx.await.wrap_err("thumbnail task died or something")??;
    for out_file in out_files {
        out_file.flush_to_storage().await?;
    }
    Ok(result)
}
