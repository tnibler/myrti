use camino::Utf8PathBuf as PathBuf;
use eyre::{Context, Result};
use tracing::instrument;

use crate::{
    core::storage::{Storage, StorageCommandOutput, StorageProvider},
    interact,
    model::{
        repository::{self, album_thumbnail::InsertAlbumThumbnail, db::PooledDbConn},
        AlbumId, AlbumThumbnailId, AssetId, AssetType,
    },
    processing::{
        self,
        commands::GenerateThumbnail,
        image::thumbnail::{GenerateThumbnailTrait, ThumbnailParams},
    },
};

#[derive(Debug, Clone)]
pub struct CreateAlbumThumbnail {
    pub album_id: AlbumId,
    pub asset_id: AssetId,
    pub size: i32,
}

#[derive(Debug, Clone)]
pub struct CreateAlbumThumbnailWithPaths {
    pub album_id: AlbumId,
    pub asset_id: AssetId,
    pub size: i32,
    pub webp_key: String,
    pub avif_key: String,
}

#[instrument(skip(conn, storage))]
pub async fn perform_side_effects_create_thumbnail(
    storage: &Storage,
    conn: &mut PooledDbConn,
    op: CreateAlbumThumbnailWithPaths,
) -> Result<()> {
    let (in_path, asset) = interact!(conn, move |conn| {
        let in_path = repository::asset::get_asset_path_on_disk(conn, op.asset_id)?.path_on_disk();
        let asset = repository::asset::get_asset(conn, op.asset_id)?;
        Ok::<_, eyre::Report>((in_path, asset))
    })
    .await??;
    create_thumbnail(
        in_path.clone(),
        asset.base.ty,
        &op.webp_key,
        &op.avif_key,
        op.size,
        storage,
    )
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct CreateAlbumThumbnailResult {
    pub thumbnail_ids: Vec<AlbumThumbnailId>,
}

#[instrument(skip(conn))]
pub async fn apply_create_thumbnail(
    conn: &mut PooledDbConn,
    create_thumbnail: CreateAlbumThumbnailWithPaths,
) -> Result<CreateAlbumThumbnailResult> {
    let mut ids: Vec<AlbumThumbnailId> = Vec::default();
    for (format, file_key) in [
        ("webp", create_thumbnail.webp_key),
        ("avif", create_thumbnail.avif_key),
    ] {
        let id = interact!(conn, move |conn| {
            repository::album_thumbnail::insert_album_thumbnail(
                conn,
                InsertAlbumThumbnail {
                    album_id: create_thumbnail.album_id,
                    format_name: format.to_string(),
                    size: create_thumbnail.size,
                    file_key: file_key.clone(),
                },
            )
        })
        .await??;
        ids.push(id);
    }
    Ok(CreateAlbumThumbnailResult { thumbnail_ids: ids })
}

#[instrument(skip(storage))]
async fn create_thumbnail(
    asset_path: PathBuf,
    asset_type: AssetType,
    webp_key: &str,
    avif_key: &str,
    size: i32,
    storage: &Storage,
) -> Result<()> {
    let out_file_avif = storage.new_command_out_file(avif_key).await?;
    let out_file_webp = storage.new_command_out_file(webp_key).await?;
    let out_dimension = processing::image::OutDimension::Crop {
        width: size,
        height: size,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let out_paths = vec![&out_file_avif, &out_file_webp];
    let thumbnail_params = ThumbnailParams {
        in_path: asset_path,
        outputs: out_paths,
        out_dimension,
    };
    let res = match asset_type {
        AssetType::Image => GenerateThumbnail::generate_thumbnail(thumbnail_params).await,
        AssetType::Video => GenerateThumbnail::generate_video_thumbnail(thumbnail_params).await,
    };
    tx.send(res).unwrap();
    let _ = rx.await.wrap_err("thumbnail task died or something")??;
    out_file_webp.flush_to_storage().await?;
    out_file_avif.flush_to_storage().await?;
    Ok(())
}
