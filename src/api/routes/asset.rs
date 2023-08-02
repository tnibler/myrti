use axum::{
    body::StreamBody,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use eyre::{bail, eyre};
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use tracing::{debug, instrument};

use crate::{
    api::{
        self,
        schema::{Asset, AssetId},
        ApiResult,
    },
    app_state::SharedState,
    model::{self, repository},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_all_assets))
        .route("/:id", get(get_asset))
        .route("/thumbnail/:id/:size/:format", get(get_thumbnail))
}

async fn get_all_assets(State(app_state): State<SharedState>) -> ApiResult<Json<Vec<Asset>>> {
    let assets: Vec<api::schema::Asset> = repository::asset::get_assets(&app_state.pool)
        .await?
        .into_iter()
        .map(|a| a.into())
        .collect();
    Ok(Json(assets))
}

async fn get_asset(
    Path(id): Path<String>,
    State(app_state): State<SharedState>,
) -> ApiResult<Json<Asset>> {
    Err(eyre!("not implemented"))?
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ThumbnailSize {
    Small,
    Large,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ThumbnailFormat {
    Jpg,
    Webp,
}

#[axum::debug_handler]
#[instrument(name = "Get Asset thumbnail", skip(app_state))]
async fn get_thumbnail(
    Path((id, size, format)): Path<(String, ThumbnailSize, ThumbnailFormat)>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    debug!("get thumbnail");
    let id: model::AssetId = AssetId(id).try_into()?;
    let asset_base = repository::asset::get_asset_base(&app_state.pool, id).await?;
    let thumb_resource = match (size, format) {
        (ThumbnailSize::Small, ThumbnailFormat::Jpg) => asset_base.thumb_small_square_jpg,
        (ThumbnailSize::Small, ThumbnailFormat::Webp) => asset_base.thumb_small_square_webp,
        (ThumbnailSize::Large, ThumbnailFormat::Jpg) => asset_base.thumb_large_orig_jpg,
        (ThumbnailSize::Large, ThumbnailFormat::Webp) => asset_base.thumb_large_orig_webp,
    };
    match thumb_resource {
        None => Ok(().into_response()),
        Some(resource_id) => {
            let resource_resolved =
                repository::resource_file::get_resource_file_resolved(&app_state.pool, resource_id)
                    .await?;
            let file = tokio::fs::File::open(resource_resolved.path_on_disk()).await?;
            let stream = ReaderStream::new(file);
            let body = StreamBody::new(stream);
            Ok(body.into_response())
        }
    }
}
