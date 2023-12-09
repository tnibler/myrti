use std::{collections::HashMap, ffi::OsString, os::unix::prelude::OsStrExt};

use axum::{
    body::StreamBody,
    extract::{Path, Query, State},
    http::{
        header::{self, CONTENT_TYPE},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use axum_extra::body::AsyncReadBody;
use chrono::Utc;
use eyre::{eyre, Context};
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use tracing::{debug, instrument, warn, Instrument};
use utoipa::ToSchema;

use core::{
    catalog::storage_key,
    core::storage::{StorageProvider, StorageReadError},
    model::{self, repository},
};

use crate::{
    app_state::SharedState,
    http_error::{ApiResult, HttpError},
    schema::{asset::Asset, AssetId},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_all_assets))
        .route("/:id", get(get_asset))
        .route("/thumbnail/:id/:size/:format", get(get_thumbnail))
        .route("/original/:id", get(get_asset_file))
        .route("/timeline", get(super::timeline::get_timeline))
        .route(
            "/repr/:asset_id/:repr_id",
            get(get_image_asset_representation),
        )
}

#[utoipa::path(get, path = "/api/asset",
responses(
    (status = 200, body=[Asset])
        ),
)]
async fn get_all_assets(State(app_state): State<SharedState>) -> ApiResult<Json<Vec<Asset>>> {
    let assets: Vec<Asset> = repository::asset::get_assets(&app_state.pool)
        .await?
        .into_iter()
        .map(|a| a.into())
        .collect();
    Ok(Json(assets))
}

#[utoipa::path(get, path = "/api/asset/{id}",
responses(
    (status = 200, body = Asset),
    (status = NOT_FOUND, description = "Asset not found")
),
    params(
        ("id" = String, Path, description = "AssetId")
    )
)
]
async fn get_asset(
    Path(_id): Path<String>,
    State(_app_state): State<SharedState>,
) -> ApiResult<Json<Asset>> {
    Err(eyre!("not implemented"))?
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ThumbnailSize {
    Small,
    Large,
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ThumbnailFormat {
    Avif,
    Webp,
}

#[instrument(name = "Get Asset thumbnail", skip(app_state))]
#[utoipa::path(get, path = "/api/thumbnail/{id}/{size}/{format}",
responses(
    (status = 200, body=String, content_type = "application/octet")
        ),
    params(
        ("id" = String, Path, description = "AssetId to get thumbnail for"),
        ("size" = ThumbnailSize, Path, description = "Thumbnail size"),
        ("format" = ThumbnailFormat, Path, description = "Image format for thumbnail")
    )
)]
async fn get_thumbnail(
    Path((id, size, format)): Path<(String, ThumbnailSize, ThumbnailFormat)>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    let asset_id: model::AssetId = AssetId(id).try_into()?;
    let (thumb_key, content_type) = match (size, format) {
        (ThumbnailSize::Small, ThumbnailFormat::Avif) => (
            storage_key::thumbnail(
                asset_id,
                model::ThumbnailType::SmallSquare,
                model::ThumbnailFormat::Avif,
            ),
            "image/avif",
        ),
        (ThumbnailSize::Small, ThumbnailFormat::Webp) => (
            storage_key::thumbnail(
                asset_id,
                model::ThumbnailType::SmallSquare,
                model::ThumbnailFormat::Webp,
            ),
            "image/webp",
        ),
        (ThumbnailSize::Large, ThumbnailFormat::Avif) => (
            storage_key::thumbnail(
                asset_id,
                model::ThumbnailType::LargeOrigAspect,
                model::ThumbnailFormat::Avif,
            ),
            "image/avif",
        ),
        (ThumbnailSize::Large, ThumbnailFormat::Webp) => (
            storage_key::thumbnail(
                asset_id,
                model::ThumbnailType::LargeOrigAspect,
                model::ThumbnailFormat::Webp,
            ),
            "image/webp",
        ),
    };
    let read = app_state.storage.open_read_stream(&thumb_key).await;
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
    return Ok((headers, body).into_response());
}

#[utoipa::path(get, path = "/api/original/{id}",
responses(
    (status = 200, body=String, content_type = "application/octet"),
    (status = NOT_FOUND, body=String, description = "Asset not found")
        ),
    params(
        ("id" = String, Path, description = "AssetId"),
    )
)]
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
    let content_type = guess_mime_type_path(&path);
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

#[utoipa::path(get, path = "/api/repr/{assetId}/{reprId}",
responses(
    (status = 200, body=String, content_type = "application/octet"),
    (status = NOT_FOUND, body=String, description = "Asset or Representation not found")
        ),
    params(
        ("assetId" = String, Path, description = "AssetId"),
        ("reprId" = String, Path, description = "ImageRepresentationId"),
    )
)]
async fn get_image_asset_representation(
    Path((asset_id, repr_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    let asset_id = model::AssetId(asset_id.parse().wrap_err("invalid asset_id")?);
    let repr_id = model::ImageRepresentationId(repr_id.parse().wrap_err("invalid repr_id")?);
    // removing format name/file extension from storage key would make this query unnecessary but
    // it's nice to have for now
    // Or maybe not since we need to set a MIME type?
    let repr = repository::representation::get_image_representation(&app_state.pool, repr_id)
        .await
        .wrap_err("no such repr_id")?;
    let storage_key = repr.file_key;
    let read_stream = app_state
        .storage
        .open_read_stream(&storage_key)
        .await
        .wrap_err("error opening read stream")?;
    let stream = ReaderStream::new(read_stream);
    let body = StreamBody::new(stream);
    let download = query
        .get("download")
        .map(|s| s.to_lowercase() == "true")
        .unwrap_or(false);
    let mut headers = HeaderMap::new();

    let file_name = format!("{}.{}", repr.asset_id.0, &repr.format_name);
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

    let content_type = guess_mime_type(&repr.format_name);
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

fn guess_mime_type(ext: &str) -> Option<&'static str> {
    match ext {
        "mp4" => Some("video/mp4"),
        "avif" => Some("image/avif"),
        "webp" => Some("image/webp"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "heif" => Some("image/heif"),
        "heic" => Some("image/heic"),
        _ => None,
    }
}

fn guess_mime_type_path(path: &camino::Utf8Path) -> Option<&'static str> {
    let ext = path.extension()?.to_ascii_lowercase();
    match guess_mime_type(&ext) {
        Some(m) => Some(m),
        None => {
            warn!(
                "can't guess MIME type for filename '{}'",
                &path
                    .file_name()
                    .map(|p| p.to_string())
                    .unwrap_or(String::new())
            );
            None
        }
    }
}
