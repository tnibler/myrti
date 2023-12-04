use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};

use crate::{
    api::{
        schema::{Album, AssetId},
        ApiResult,
    },
    app_state::SharedState,
    model::{self, repository},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_all_albums))
        .route("/", post(post_create_album))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAlbumRequest {
    pub is_timeline_group: bool,
    pub name: String,
    pub description: Option<String>,
    pub assets: Vec<AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAlbumResponse {
    pub album_id: i64,
}

#[tracing::instrument(skip(app_state))]
pub async fn get_all_albums(State(app_state): State<SharedState>) -> ApiResult<Json<Vec<Album>>> {
    let albums: Vec<Album> = repository::album::get_all_albums_with_asset_count(&app_state.pool)
        .await?
        .into_iter()
        .map(|(album, num_assets)| Album::from_model(&album, num_assets))
        .collect();
    Ok(Json(albums))
}

#[tracing::instrument(skip(app_state))]
pub async fn post_create_album(
    State(app_state): State<SharedState>,
    Json(request): Json<CreateAlbumRequest>,
) -> ApiResult<Json<CreateAlbumResponse>> {
    if request.name.is_empty() {
        return Err(eyre!("name can not be empty").into());
    }
    let create_timeline_group = if request.is_timeline_group {
        // use last asset date for now
        let last_asset_id = match request.assets.last() {
            None => return Err(eyre!("can not create empty timeline group").into()),
            Some(a) => model::AssetId::try_from(a)?,
        };
        let last_asset = repository::asset::get_asset(&app_state.pool, last_asset_id).await?;
        Some(repository::album::CreateTimelineGroup {
            display_date: last_asset.base.taken_date,
        })
    } else {
        None
    };
    let create_album = repository::album::CreateAlbum {
        name: Some(request.name),
        description: request.description,
        timeline_group: create_timeline_group,
    };
    let asset_ids: Vec<model::AssetId> = request
        .assets
        .into_iter()
        .map(|id| id.try_into())
        .collect::<Result<Vec<_>>>()?;
    let album_id =
        repository::album::create_album(&app_state.pool, create_album, &asset_ids).await?;
    Ok(Json(CreateAlbumResponse {
        album_id: album_id.0,
    }))
}
