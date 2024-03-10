use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use tracing::Instrument;
use utoipa::{IntoParams, ToSchema};

use core::{
    deadpool_diesel, interact,
    model::{self, repository},
};

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{Album, AssetId},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_all_albums))
        .route("/", post(create_album))
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
#[tracing::instrument(skip(app_state))]
pub async fn get_all_albums(State(app_state): State<SharedState>) -> ApiResult<Json<Vec<Album>>> {
    let conn = app_state.pool.get().in_current_span().await?;
    let albums: Vec<Album> = interact!(conn, move |mut conn| {
        repository::album::get_all_albums_with_asset_count(&mut conn)
    })
    .in_current_span()
    .await??
    .into_iter()
    .map(|(album, num_assets)| Album::from_model(&album, num_assets))
    .collect();
    Ok(Json(albums))
}

#[utoipa::path(
    post,
    path = "/api/albums",
    params(CreateAlbumRequest),
    responses((status = 200, body=CreateAlbumResponse)),
)]
#[tracing::instrument(skip(app_state))]
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
    let conn = app_state.pool.get().in_current_span().await?;
    let album_id = interact!(conn, move |mut conn| {
        repository::album::create_album(&mut conn, create_album, &asset_ids)
    })
    .in_current_span()
    .await??;
    Ok(Json(CreateAlbumResponse {
        album_id: album_id.0,
    }))
}
