use axum::{
    extract::{Path, State},
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use axum_extra::body::AsyncReadBody;
use eyre::{eyre, Context, Result};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use core::{
    catalog::storage_key,
    core::storage::{StorageProvider, StorageReadError},
    deadpool_diesel, interact,
    model::{self, repository},
};

use crate::{
    app_state::SharedState,
    http_error::{ApiResult, HttpError},
    schema::{
        asset::{AssetSpe, AssetWithSpe, Image, Video},
        Album, AlbumId, AlbumItemId, AssetId,
    },
};

use super::asset::ThumbnailFormat;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_all_albums))
        .route("/", post(create_album))
        .route("/:id/assets", put(append_assets_to_album))
        .route("/:id", get(get_album_details))
        .route("/:id/thumbnail/:size/:format", get(get_album_thumbnail))
        .route("/:id/deleteItems", post(delete_album_items))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CreateAlbumRequest {
    pub name: String,
    pub description: Option<String>,
    pub assets: Vec<AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateAlbumResponse {
    pub album_id: i64,
}

#[utoipa::path(
    get,
    path = "/api/albums",
    responses((status = 200, body=Vec<Album>)),
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
pub async fn get_all_albums(State(app_state): State<SharedState>) -> ApiResult<Json<Vec<Album>>> {
    let conn = app_state.pool.get().await?;
    let albums: Vec<Album> = interact!(conn, move |conn| {
        repository::album::get_all_albums_with_asset_count(conn)
    })
    .await??
    .into_iter()
    .map(|(album, num_assets)| Album::from_model(&album, num_assets))
    .collect();
    Ok(Json(albums))
}

#[utoipa::path(
    post,
    path = "/api/albums",
    request_body = CreateAlbumRequest,
    responses((status = 200, body=CreateAlbumResponse)),
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
pub async fn create_album(
    State(app_state): State<SharedState>,
    Json(request): Json<CreateAlbumRequest>,
) -> ApiResult<Json<CreateAlbumResponse>> {
    if request.name.is_empty() {
        return Err(eyre!("name can not be empty").into());
    }
    let create_album = repository::album::CreateAlbum {
        name: Some(request.name),
        description: request.description,
    };
    let asset_ids: Vec<model::AssetId> = request
        .assets
        .into_iter()
        .map(|id| id.try_into())
        .collect::<Result<Vec<_>>>()?;
    let conn = app_state.pool.get().await?;
    let album_id = interact!(conn, move |conn| {
        repository::album::create_album(conn, create_album, &asset_ids)
    })
    .await??;
    Ok(Json(CreateAlbumResponse {
        album_id: album_id.0,
    }))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "itemType", rename_all = "camelCase")]
pub struct AlbumItem {
    pub item_id: AlbumItemId,
    #[serde(flatten)]
    pub ty: AlbumItemType,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "itemType", rename_all = "camelCase")]
pub enum AlbumItemType {
    // not a tuple struct because that doesn't work with utoipa
    Asset { asset: AssetWithSpe },
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDetailsResponse {
    pub items: Vec<AlbumItem>,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/albums/{id}",
    responses((status = 200, body=AlbumDetailsResponse)),
    params(
        ("id"=String, description="Album id")
    )
)]
#[tracing::instrument(fields(request = true), err, skip(app_state))]
pub async fn get_album_details(
    Path(album_id): Path<AlbumId>,
    State(app_state): State<SharedState>,
) -> ApiResult<Json<AlbumDetailsResponse>> {
    let album_id: model::AlbumId = album_id.try_into()?;
    let conn = app_state.pool.get().await?;
    let items = interact!(conn, move |conn| {
        repository::album::get_items_in_album(conn, album_id)
    })
    .await??;
    let items: Vec<_> = items
        .into_iter()
        .map(|item| AlbumItem {
            item_id: item.id.into(),
            ty: match item.item {
                model::AlbumItemType::Asset(asset) => AlbumItemType::Asset {
                    asset: AssetWithSpe {
                        spe: match &asset.sp {
                            model::AssetSpe::Image(_image) => AssetSpe::Image(Image {
                                representations: Vec::default(), //FIXME
                            }),
                            model::AssetSpe::Video(video) => AssetSpe::Video(Video {
                                has_dash: video.has_dash,
                            }),
                        },
                        asset: asset.into(),
                    },
                },
                model::AlbumItemType::Text(text) => AlbumItemType::Text { text },
            },
        })
        .collect();
    let album = interact!(conn, move |conn| {
        repository::album::get_album(conn, album_id)
    })
    .await??;
    Ok(Json(AlbumDetailsResponse {
        name: album.name,
        description: album.description,
        items,
    }))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppendAssetsResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct AppendAssetsRequest {
    pub asset_ids: Vec<AssetId>,
}

#[utoipa::path(
    put,
    path = "/api/albums/{id}/assets",
    request_body = AppendAssetsRequest,
    responses((status = 200, body=AppendAssetsResponse)),
    params(
        ("id"=String, description="Album id")
    )
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
pub async fn append_assets_to_album(
    Path(album_id): Path<AlbumId>,
    State(app_state): State<SharedState>,
    Json(req): Json<AppendAssetsRequest>,
) -> ApiResult<Json<AppendAssetsResponse>> {
    let album_id: model::AlbumId = album_id.try_into()?;
    let asset_ids: Vec<_> = req
        .asset_ids
        .into_iter()
        .map(|id| {
            id.0.parse::<i64>()
                .wrap_err("invalid assetId")
                .map(model::AssetId)
        })
        .collect::<Result<_>>()?;
    if asset_ids.is_empty() {
        return Ok(Json(AppendAssetsResponse { success: true }));
    }
    let conn = app_state.pool.get().await?;
    let append_result = interact!(conn, move |conn| {
        repository::album::append_assets_to_album(conn, album_id, &asset_ids)
    })
    .await?
    .wrap_err("Error appending assets to album");
    if let Err(err) = append_result {
        tracing::warn!(?err);
        Err(err.into())
    } else {
        Ok(Json(AppendAssetsResponse { success: true }))
    }
}

#[utoipa::path(get, path = "/api/albums/{id}/thumbnail/{size}/{format}",
responses(
    (status = 200, body=String, content_type = "application/octet")
        ),
    params(
        ("id" = String, Path, description = "AlbumId to get thumbnail for"),
        ("size" = ThumbnailSize, Path, description = "Thumbnail size"),
        ("format" = ThumbnailFormat, Path, description = "Image format for thumbnail")
    )
)]
pub async fn get_album_thumbnail(
    Path((album_id, _size, format)): Path<(AlbumId, String, ThumbnailFormat)>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    let album_id: model::AlbumId = album_id.try_into()?;
    // TODO dedupe this, same thing is required for asset thumbnails and image reprs
    let (format, content_type) = match format {
        ThumbnailFormat::Avif => (model::ThumbnailFormat::Avif, "image/avif"),
        ThumbnailFormat::Webp => (model::ThumbnailFormat::Webp, "image/webp"),
    };
    let file_key = storage_key::album_thumbnail(album_id, format);
    let read = app_state.storage.open_read_stream(&file_key).await;
    let read = match read {
        Err(err) => match err {
            StorageReadError::FileNotFound(_) => {
                return Ok((
                    StatusCode::NOT_FOUND,
                    HttpError::from(eyre!("no such object")),
                )
                    .into_response());
            }
            _ => {
                return Err(eyre!("could not open object for reading").into());
            }
        },
        Ok(r) => r,
    };
    let headers = [(CONTENT_TYPE, content_type)];
    let body = AsyncReadBody::new(read);
    // TODO add size hint for files https://github.com/tokio-rs/axum/discussions/2074
    Ok((headers, body).into_response())
}

#[derive(Debug, Clone, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAlbumItemRequest {
    pub item_ids: Vec<AlbumItemId>,
}

#[utoipa::path(
    post,
    path = "/api/albums/{id}/deleteItems",
    request_body = DeleteAlbumItemRequest,
    responses((status = 200, body=())),
    params(
        ("id"=String, description="Album id")
    )
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
pub async fn delete_album_items(
    Path(album_id): Path<AlbumId>,
    State(app_state): State<SharedState>,
    Json(req): Json<DeleteAlbumItemRequest>,
) -> ApiResult<()> {
    let conn = app_state.pool.get().await?;
    let item_ids: Vec<model::AlbumItemId> = req
        .item_ids
        .into_iter()
        .map(|id| Ok(model::AlbumItemId(id.0.parse()?)))
        .collect::<Result<Vec<_>>>()
        .wrap_err("bad item ids")?;
    let album_id: model::AlbumId = album_id.try_into()?;
    interact!(conn, move |conn| {
        repository::album::remove_items_from_album(conn, album_id, &item_ids)
            .wrap_err("error removing items from album")
    })
    .await??;
    Ok(())
}
