use std::{collections::HashMap, ffi::OsString, ops::Deref, os::unix::prelude::OsStrExt};

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{
        header::{self, CONTENT_TYPE},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::body::AsyncReadBody;
use eyre::{eyre, Context, Result};
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use utoipa::ToSchema;

use core::{
    catalog::storage_key,
    core::storage::{StorageProvider, StorageReadError},
    deadpool_diesel, interact,
    model::{self, repository},
};

use crate::{
    app_state::SharedState,
    http_error::{ApiResult, HttpError},
    mime_type::{guess_mime_type, guess_mime_type_path},
    schema::{asset::Asset, AssetId, ImageRepresentationId},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_all_assets))
        .route("/:id", get(get_asset))
        .route("/:id/details", get(get_asset_details))
        .route("/thumbnail/:id/:size/:format", get(get_thumbnail))
        .route("/original/:id", get(get_asset_file))
        .route("/timeline", get(super::timeline::get_timeline))
        .route("/hidden", post(set_assets_hidden))
        .route(
            "/repr/:asset_id/:repr_id",
            get(get_image_asset_representation),
        )
        .route("/:id/rotation", post(set_asset_rotation_correction))
}

#[utoipa::path(get, path = "/api/assets",
    responses(
        (status = 200, body=[Asset])
    ),
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
async fn get_all_assets(State(app_state): State<SharedState>) -> ApiResult<Json<Vec<Asset>>> {
    let conn = app_state.pool.get().await?;
    let assets: Vec<Asset> = interact!(conn, move |conn| { repository::asset::get_assets(conn) })
        .await??
        .into_iter()
        .map(|a| a.into())
        .collect();
    Ok(Json(assets))
}

#[utoipa::path(get, path = "/api/assets/{id}",
    responses(
        (status = 200, body = Asset),
        (status = NOT_FOUND, description = "Asset not found")
    ),
    params(
        ("id" = String, Path, description = "AssetId")
    )
)]
async fn get_asset(
    Path(_id): Path<String>,
    State(_app_state): State<SharedState>,
) -> ApiResult<Json<Asset>> {
    Err(eyre!("not implemented"))?
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetDetailsResponse {
    pub exiftool_output: serde_json::Value,
}

#[utoipa::path(get, path = "/api/assets/{id}/details",
    responses(
        (status = 200, body = AssetDetailsResponse),
        (status = NOT_FOUND, description = "Asset not found")
    ),
    params(
        ("id" = String, Path, description = "AssetId")
    )
)]
async fn get_asset_details(
    Path(asset_id): Path<AssetId>,
    State(app_state): State<SharedState>,
) -> ApiResult<Json<AssetDetailsResponse>> {
    let asset_id: model::AssetId = asset_id.try_into()?;
    let conn = app_state.pool.get().await?;
    let exiftool_output = interact!(conn, move |conn| {
        repository::asset::get_asset_exiftool_output(conn, asset_id)
    })
    .await??;
    let json = match serde_json::from_slice(&exiftool_output)
        .wrap_err("failed to parse JSON exiftool_output")?
    {
        // raw exiftool output in db is an array with a single element [{/*...*/}],
        // so remove that wrapping array
        serde_json::Value::Array(mut inner) if inner.len() == 1 => {
            Ok(inner.pop().expect("length was checked to be 1"))
        }

        other => Err(eyre!("unexpected in JSON exiftool output: {:?}", other)),
    }?;
    Ok(Json(AssetDetailsResponse {
        exiftool_output: json,
    }))
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

#[utoipa::path(get, path = "/api/assets/thumbnail/{id}/{size}/{format}",
    responses(
        (status = 200, body=String, content_type = "application/octet")
    ),
    params(
        ("id" = String, Path, description = "AssetId to get thumbnail for"),
        ("size" = ThumbnailSize, Path, description = "Thumbnail size"),
        ("format" = ThumbnailFormat, Path, description = "Image format for thumbnail")
    )
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
async fn get_thumbnail(
    Path((asset_id, size, format)): Path<(AssetId, ThumbnailSize, ThumbnailFormat)>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    let asset_id: model::AssetId = asset_id.try_into()?;
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

#[utoipa::path(get, path = "/api/assets/original/{id}",
    responses(
        (status = 200, body=String, content_type = "application/octet"),
        (status = NOT_FOUND, body=String, description = "Asset not found")
    ),
    params(
        ("id" = String, Path, description = "AssetId"),
    )
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
async fn get_asset_file(
    Path(asset_id): Path<AssetId>,
    Query(query): Query<HashMap<String, String>>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    let id: model::AssetId = asset_id.try_into()?;
    let conn = app_state.pool.get().await?;
    let path = interact!(conn, move |conn| {
        repository::asset::get_asset_path_on_disk(conn, id)
    })
    .await??
    .path_on_disk();
    let file = tokio::fs::File::open(&path).await?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
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
                .deref()
                .try_into()
                .wrap_err("error setting content-type header")?,
        );
    }
    Ok((headers, body).into_response())
}

#[utoipa::path(get, path = "/api/assets/repr/{assetId}/{reprId}",
    responses(
        (status = 200, body=String, content_type = "application/octet"),
        (status = NOT_FOUND, body=String, description = "Asset or Representation not found")
    ),
    params(
        ("assetId" = String, Path, description = "AssetId"),
        ("reprId" = String, Path, description = "ImageRepresentationId"),
    )
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
async fn get_image_asset_representation(
    Path((asset_id, repr_id)): Path<(AssetId, ImageRepresentationId)>,
    Query(query): Query<HashMap<String, String>>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    let repr_id: model::ImageRepresentationId = repr_id.try_into()?;
    // removing format name/file extension from storage key would make this query unnecessary but
    // it's nice to have for now
    // Or maybe not since we need to set a MIME type?
    let conn = app_state.pool.get().await?;
    let repr = interact!(conn, move |conn| {
        repository::representation::get_image_representation(conn, repr_id)
    })
    .await?
    .wrap_err("no such repr_id")?;
    let storage_key = repr.file_key;
    let read_stream = app_state
        .storage
        .open_read_stream(&storage_key)
        .await
        .wrap_err("error opening read stream")?;
    let stream = ReaderStream::new(read_stream);
    let body = Body::from_stream(stream);
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
                .deref()
                .try_into()
                .wrap_err("error setting content-type header")?,
        );
    }
    Ok((headers, body).into_response())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum HideAssetAction {
    Hide,
    Unhide,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HideAssetsRequest {
    pub what: HideAssetAction,
    pub asset_ids: Vec<AssetId>,
}

#[utoipa::path(
    post,
    path = "/api/assets/hidden",
    request_body=HideAssetsRequest,
    responses((status=200)),
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
async fn set_assets_hidden(
    State(app_state): State<SharedState>,
    Json(req): Json<HideAssetsRequest>,
) -> ApiResult<()> {
    let asset_ids: Vec<model::AssetId> = req
        .asset_ids
        .into_iter()
        .map(model::AssetId::try_from)
        .collect::<Result<Vec<_>>>()?;
    let conn = app_state.pool.get().await?;
    interact!(conn, move |conn| {
        repository::asset::set_assets_hidden(conn, true, &asset_ids)
    })
    .await?
    .wrap_err("error setting Assets hidden")?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetAssetRotationRequest {
    pub rotation: Option<i32>,
}

#[utoipa::path(
    post,
    path = "/api/assets/rotation",
    request_body=SetAssetRotationRequest,
    responses((status=200))
)]
async fn set_asset_rotation_correction(
    State(app_state): State<SharedState>,
    Path(asset_id): Path<AssetId>,
    Json(req): Json<SetAssetRotationRequest>,
) -> ApiResult<()> {
    match req.rotation {
        Some(rot) if rot % 90 != 0 => Err(eyre!("Invalid rotation value").into()),
        rotation => {
            let asset_id: model::AssetId = asset_id.try_into()?;
            let conn = app_state.pool.get().await?;
            interact!(conn, move |conn| {
                repository::asset::set_asset_rotation_correction(conn, asset_id, rotation)
            })
            .await??;
            Ok(())
        }
    }
}
