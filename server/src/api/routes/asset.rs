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
use chrono::{DateTime, Utc};
use eyre::{eyre, Context};
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use tracing::{debug, instrument, warn, Instrument};

use crate::{
    api::{
        self,
        schema::{Asset, AssetId, AssetWithSpe, TimelineChunk, TimelineGroupType, TimelineRequest},
        ApiResult,
    },
    app_state::SharedState,
    catalog::storage_key,
    core::storage::{StorageProvider, StorageReadError},
    http_error::HttpError,
    model::{
        self,
        repository::{self, pool::DbPool, timeline::TimelineElement},
    },
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_all_assets))
        .route("/:id", get(get_asset))
        .route("/thumbnail/:id/:size/:format", get(get_thumbnail))
        .route("/original/:id", get(get_asset_file))
        .route("/timeline", get(get_timeline))
        .route(
            "/repr/:asset_id/:repr_id",
            get(get_image_asset_representation),
        )
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
    Path(_id): Path<String>,
    State(_app_state): State<SharedState>,
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
    Avif,
    Webp,
}

#[instrument(name = "Get Asset thumbnail", skip(app_state))]
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
    let storage_key = storage_key::image_representation(asset_id, repr_id, &repr.format_name);
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

#[instrument(skip(app_state))]
async fn get_timeline(
    State(app_state): State<SharedState>,
    Query(req_body): Query<TimelineRequest>,
) -> ApiResult<Json<TimelineChunk>> {
    debug!(?req_body);
    let now = Utc::now();
    let last_asset_id = match req_body.last_asset_id {
        Some(s) => Some(model::AssetId(s.parse().wrap_err("bad asset id")?)),
        None => None,
    };
    let groups = repository::timeline::get_timeline_chunk(
        &app_state.pool,
        last_asset_id,
        req_body.max_count.into(),
    )
    .in_current_span()
    .await?;
    let filtered_nonempty_groups = groups.into_iter().filter(|group| match group {
        TimelineElement::DayGrouped(assets) => !assets.is_empty(),
        TimelineElement::Group { group: _, assets } => !assets.is_empty(),
    });
    let mut api_groups: Vec<api::schema::TimelineGroup> = Vec::default();
    for group in filtered_nonempty_groups {
        let mut api_assets_with_spe: Vec<api::schema::AssetWithSpe> = Vec::default();
        let assets = match &group {
            TimelineElement::DayGrouped(assets) => assets,
            TimelineElement::Group { group: _, assets } => assets,
        };
        for asset in assets {
            api_assets_with_spe.push(asset_with_spe(&app_state.pool, asset).await?);
        }
        let api_group = match group {
            TimelineElement::DayGrouped(assets) => api::schema::TimelineGroup {
                ty: TimelineGroupType::Day(assets.last().unwrap().base.taken_date),
                assets: api_assets_with_spe,
            },
            TimelineElement::Group { group, assets } => api::schema::TimelineGroup {
                ty: TimelineGroupType::Group {
                    title: group.album.name.unwrap_or(String::from("NONAME")),
                    // unwrap is ok because empty asset vecs are filtered out above
                    start: assets.first().unwrap().base.taken_date,
                    // FIXME these should maybe not be UTC but local dates
                    end: assets.last().unwrap().base.taken_date,
                },
                assets: api_assets_with_spe,
            },
        };
        api_groups.push(api_group);
    }
    Ok(Json(TimelineChunk {
        date: now,
        changed_since_last_fetch: false,
        groups: api_groups,
    }))
}

async fn asset_with_spe(
    pool: &DbPool,
    asset: &model::Asset,
) -> eyre::Result<api::schema::AssetWithSpe> {
    match &asset.sp {
        model::AssetSpe::Image(_image) => {
            let reprs =
                repository::representation::get_image_representations(pool, asset.base.id).await?;
            let api_reprs = reprs
                .into_iter()
                .map(|repr| api::schema::ImageRepresentation {
                    id: repr.id.0.to_string(),
                    format: repr.format_name,
                    width: repr.width,
                    height: repr.height,
                    size: repr.file_size,
                })
                .collect();
            Ok(AssetWithSpe {
                asset: asset.into(),
                spe: api::schema::AssetSpe::Image(api::schema::Image {
                    representations: api_reprs,
                }),
            })
        }
        model::AssetSpe::Video(_video) => Ok(AssetWithSpe {
            asset: asset.into(),
            spe: api::schema::AssetSpe::Video(api::schema::Video {}),
        }),
    }
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
