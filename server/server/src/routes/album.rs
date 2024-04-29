use axum::{
    extract::{Path, State},
    routing::{get, post, put},
    Json, Router,
};
use eyre::{eyre, Context, Result};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use core::{
    deadpool_diesel, interact,
    model::{self, repository, AlbumId},
};

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{asset::Asset, Album, AssetId},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_all_albums))
        .route("/", post(create_album))
        .route("/:id/assets", put(append_assets_to_album))
        .route("/:id", get(get_album_details))
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
#[serde(tag = "albumItemType")]
pub enum AlbumItem {
    Asset(Asset),
    Text { text: String }, // not a tuple struct because that doesn't work with utoipa (String field is not
                           // generated, only type tag)
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
    responses((status = 200, body=AlbumDetailsResponse))
)]
#[tracing::instrument(fields(request = true), err, skip(app_state))]
pub async fn get_album_details(
    Path(album_id): Path<String>,
    State(app_state): State<SharedState>,
) -> ApiResult<Json<AlbumDetailsResponse>> {
    let album_id = album_id.parse().wrap_err("Invalid albumId").map(AlbumId)?;
    let conn = app_state.pool.get().await?;
    let items = interact!(conn, move |conn| {
        repository::album::get_items_in_album(conn, album_id)
    })
    .await??;
    let items: Vec<_> = items
        .into_iter()
        .map(|item| match item.item {
            model::AlbumItemType::Asset(asset) => AlbumItem::Asset(asset.into()),
            model::AlbumItemType::Text(text) => AlbumItem::Text { text },
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
    responses((status = 200, body=AppendAssetsResponse))
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
pub async fn append_assets_to_album(
    Path(album_id): Path<String>,
    State(app_state): State<SharedState>,
    Json(req): Json<AppendAssetsRequest>,
) -> ApiResult<Json<AppendAssetsResponse>> {
    let album_id = album_id.parse().wrap_err("Invalid albumId").map(AlbumId)?;
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
