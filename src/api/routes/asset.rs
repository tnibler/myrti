use std::{collections::HashMap, ffi::OsString, os::unix::prelude::OsStrExt};

use axum::{
    body::StreamBody,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use eyre::{bail, eyre, Context};
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use tracing::{debug, instrument, warn};

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
        .route("/file/:id", get(get_asset_file))
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

#[instrument(name = "Get Asset thumbnail", skip(app_state))]
async fn get_thumbnail(
    Path((id, size, format)): Path<(String, ThumbnailSize, ThumbnailFormat)>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    debug!("get thumbnail");
    let id: model::AssetId = AssetId(id).try_into()?;
    let asset_base = repository::asset::get_asset_base(&app_state.pool, id).await?;
    let (thumb_resource, content_type) = match (size, format) {
        (ThumbnailSize::Small, ThumbnailFormat::Jpg) => {
            (asset_base.thumb_small_square_jpg, "image/jpeg")
        }
        (ThumbnailSize::Small, ThumbnailFormat::Webp) => {
            (asset_base.thumb_small_square_webp, "image/webp")
        }
        (ThumbnailSize::Large, ThumbnailFormat::Jpg) => {
            (asset_base.thumb_large_orig_jpg, "image/jpeg")
        }
        (ThumbnailSize::Large, ThumbnailFormat::Webp) => {
            (asset_base.thumb_large_orig_webp, "image/webp")
        }
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
            Ok(([(header::CONTENT_TYPE, content_type)], body).into_response())
        }
    }
}

#[instrument(name = "Get Asset file", skip(app_state))]
async fn get_asset_file(
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    let id: model::AssetId = AssetId(id).try_into()?;
    let path = repository::asset::get_asset_path_on_disk(&app_state.pool, id)
        .await?
        .path_on_disk();
    let file = tokio::fs::File::open(&path).await?;
    let stream = ReaderStream::new(file);
    let body = StreamBody::new(stream);
    let download = query
        .get("download")
        .map(|s| s.to_lowercase() == "true")
        .unwrap_or(false);
    let mut headers = HeaderMap::new();
    if let Some(file_name) = path.file_name() {
        let mut s = match download {
            true => OsString::from("attachment; filename=\""),
            false => OsString::from("inline; filename=\""),
        };
        s.push(file_name);
        s.push("\"");
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_bytes(s.as_bytes())
                .wrap_err("error setting content-disposition header")?,
        );
    }
    let content_type = guess_mime_type(&path);
    if let Some(content_type) = content_type {
        headers.insert(
            header::CONTENT_TYPE,
            content_type
                .try_into()
                .wrap_err("error setting content-type header")?,
        );
    }
    Ok((headers, body).into_response())
}

fn guess_mime_type(path: &std::path::Path) -> Option<&'static str> {
    let ext = path.extension()?.to_ascii_lowercase().to_str()?.to_string();
    match ext.as_str() {
        "mp4" => Some("video/mp4"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        _ => {
            warn!(
                "can't guess MIME type for filename '{}'",
                &path
                    .file_name()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(String::new())
            );
            None
        }
    }
}
