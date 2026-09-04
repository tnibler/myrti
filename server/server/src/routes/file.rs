use std::{collections::HashMap, ffi::OsString, ops::Deref, os::unix::ffi::OsStrExt};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::body::AsyncReadBody;
use eyre::{Context, eyre};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use utoipa::ToSchema;

use myrti_core::{
    catalog::storage_key,
    core::{
        scheduler::{SchedulerMessage, UserRequest},
        storage::StorageReadError,
    },
};
use myrti_data::{interact, model, repository};

use crate::{
    app_state::SharedState,
    http_error::{ApiResult, HttpError},
    mime_type::{guess_mime_type, guess_mime_type_path},
    schema::{FileId, ImageRepresentationId, asset::MirrorCorrection},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/thumbnails/regenerate", post(regenerate_thumbnail))
        .route("/disableGhiStreaming", post(disable_ghi_streaming))
        .nest(
            "/{id}/",
            Router::new()
                .route("/repr/{repr_id}", get(get_image_asset_representation))
                .route("/details", get(get_file_details))
                .route("/thumbnail/{size}/{format}", get(get_thumbnail))
                .route("/original", get(get_original_file))
                .route("/transform", post(set_asset_transform_correction)),
        )
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetDetailsResponse {
    pub exiftool_output: serde_json::Value,
}

#[utoipa::path(get, path = "/api/files/{id}/details",
    responses(
        (status = 200, body = AssetDetailsResponse),
        (status = NOT_FOUND, description = "Asset not found")
    ),
    params(
        ("id" = String, Path, description = "FileId")
    )
)]
async fn get_file_details(
    Path(file_id): Path<FileId>,
    State(app_state): State<SharedState>,
) -> ApiResult<Json<AssetDetailsResponse>> {
    let file_id: model::FileId = file_id.try_into()?;
    let conn = app_state.pool.get().await?;
    let exiftool_output: Vec<u8> = interact!(conn, move |conn| {
        repository::asset::get_exiftool_output(conn, file_id)
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

#[utoipa::path(get, path = "/api/files/{id}/thumbnail/{size}/{format}",
    responses(
        (status = 200, body=String, content_type = "application/octet")
    ),
    params(
        ("id" = String, Path, description = "FileId to get thumbnail for"),
        ("size" = ThumbnailSize, Path, description = "Thumbnail size"),
        ("format" = ThumbnailFormat, Path, description = "Image format for thumbnail")
    )
)]
#[tracing::instrument(skip(app_state), level = "trace")]
async fn get_thumbnail(
    Path((file_id, size, format)): Path<(FileId, ThumbnailSize, ThumbnailFormat)>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    let file_id: model::FileId = file_id.try_into()?;
    let (thumb_key, content_type) = match (size, format) {
        (ThumbnailSize::Small, ThumbnailFormat::Avif) => (
            storage_key::thumbnail(
                file_id,
                model::ThumbnailType::SmallSquare,
                model::ThumbnailFormat::Avif,
            ),
            "image/avif",
        ),
        (ThumbnailSize::Small, ThumbnailFormat::Webp) => (
            storage_key::thumbnail(
                file_id,
                model::ThumbnailType::SmallSquare,
                model::ThumbnailFormat::Webp,
            ),
            "image/webp",
        ),
        (ThumbnailSize::Large, ThumbnailFormat::Avif) => (
            storage_key::thumbnail(
                file_id,
                model::ThumbnailType::LargeOrigAspect,
                model::ThumbnailFormat::Avif,
            ),
            "image/avif",
        ),
        (ThumbnailSize::Large, ThumbnailFormat::Webp) => (
            storage_key::thumbnail(
                file_id,
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
    let headers = [(header::CONTENT_TYPE, content_type)];
    let body = AsyncReadBody::new(read);
    // TODO add size hint for files https://github.com/tokio-rs/axum/discussions/2074
    return Ok((headers, body).into_response());
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateThumbnailsRequest {
    /// List of FileId, or null to regenerate thumbnails for all files
    file_ids: Option<Vec<FileId>>,
}

#[utoipa::path(post, path = "/api/files/thumbnails/regenerate",
    responses(
        (status = 200, body=()),
    ),
    request_body = RegenerateThumbnailsRequest,
)]
#[tracing::instrument(skip(app_state), level = "trace")]
async fn regenerate_thumbnail(
    State(app_state): State<SharedState>,
    Json(request): Json<RegenerateThumbnailsRequest>,
) -> ApiResult<Response> {
    let ids: Option<Vec<model::FileId>> = request
        .file_ids
        .map(|ids| ids.into_iter().map(model::FileId::try_from).try_collect())
        .transpose()?;
    app_state
        .scheduler
        .send
        .send(SchedulerMessage::UserRequest(
            UserRequest::RegenerateThumbnails(ids),
        ))
        .await
        .expect("receiver must be alive");
    Ok(().into_response())
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DisableDashGhiRequest {
    file_ids: Vec<FileId>,
}

#[utoipa::path(post, path = "/api/files/disableGhiStreaming",
    responses(
        (status = 200, body=()),
    ),
    request_body = DisableDashGhiRequest,
)]
#[tracing::instrument(skip(app_state), level = "trace")]
async fn disable_ghi_streaming(
    State(app_state): State<SharedState>,
    Json(request): Json<DisableDashGhiRequest>,
) -> ApiResult<Response> {
    let ids: Vec<model::FileId> = request
        .file_ids
        .into_iter()
        .map(model::FileId::try_from)
        .try_collect()?;
    app_state
        .scheduler
        .send
        .send(SchedulerMessage::UserRequest(
            UserRequest::DisableGhiStreaming(ids),
        ))
        .await
        .expect("receiver must be alive");
    Ok(().into_response())
}

#[utoipa::path(get, path = "/api/files/{id}/original",
    responses(
        (status = 200, body=String, content_type = "application/octet"),
        (status = NOT_FOUND, body=String, description = "File not found")
    ),
    params(
        ("id" = String, Path, description = "FileId"),
    )
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
async fn get_original_file(
    Path(file_id): Path<FileId>,
    Query(query): Query<HashMap<String, String>>,
    State(app_state): State<SharedState>,
) -> ApiResult<Response> {
    let id: model::FileId = file_id.try_into()?;
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

#[utoipa::path(get, path = "/api/files/{fileId}/repr/{reprId}",
    responses(
        (status = 200, body=String, content_type = "application/octet"),
        (status = NOT_FOUND, body=String, description = "File or Representation not found")
    ),
    params(
        ("fileId" = String, Path, description = "FileId"),
        ("reprId" = String, Path, description = "ImageRepresentationId"),
    )
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
async fn get_image_asset_representation(
    Path((file_id, repr_id)): Path<(FileId, ImageRepresentationId)>,
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

    let file_name = format!("{}.{}", repr.file_id.0, repr.format_name);
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

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetAssetRotationRequest {
    /// One of 0, 90, 180, 270
    pub rotation: Option<i32>,
    pub mirror: Option<MirrorCorrection>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetAssetRotationResponse {
    /// One of 0, 90, 180, 270
    pub rotation: i32,
    pub mirror: MirrorCorrection,
}

#[utoipa::path(
    post,
    path = "/api/files/{id}/transform",
    params(
        ("id" = String, Path, description = "FileId")
    ),
    request_body=SetAssetRotationRequest,
    responses((status=200, body = SetAssetRotationResponse))
)]
async fn set_asset_transform_correction(
    State(app_state): State<SharedState>,
    Path(file_id): Path<FileId>,
    Json(req): Json<SetAssetRotationRequest>,
) -> ApiResult<Json<SetAssetRotationResponse>> {
    let file_id: model::FileId = file_id.try_into()?;
    let conn = app_state.pool.get().await?;
    if req.rotation.is_none() && req.mirror.is_none() {
        return Err(eyre!("must set at least one field").into());
    }
    let asset_id = if let Some(rotation) = req.rotation {
        let rotation = match rotation {
            0 => model::RotationCorrection::CW0,
            90 => model::RotationCorrection::CW90,
            180 => model::RotationCorrection::CW180,
            270 => model::RotationCorrection::CW270,
            _ => return Err(eyre!("Invalid rotation value").into()),
        };
        Some(
            interact!(conn, move |conn| {
                repository::asset::set_rotation_correction_all_asset_files(conn, file_id, rotation)
            })
            .await??,
        )
    } else {
        None
    };
    let asset_id = if let Some(mirror) = req.mirror {
        interact!(conn, move |conn| {
            repository::asset::set_mirror_correction_all_asset_files(conn, file_id, mirror.into())
        })
        .await??
    } else {
        asset_id.expect("at least one query will run")
    };
    let asset = interact!(conn, move |conn| {
        repository::asset::get_asset(conn, asset_id)
    })
    .await??;
    Ok(Json(SetAssetRotationResponse {
        rotation: asset.rep_file.rotation_correction.degrees(),
        mirror: asset.rep_file.mirror_correction.into(),
    }))
}
