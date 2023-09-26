use std::{collections::HashMap, ffi::OsString, os::unix::prelude::OsStrExt};

use axum::{
    body::StreamBody,
    extract::{Path, Query, State},
    http::{
        header::{self, CONTENT_TYPE},
        HeaderMap, HeaderValue,
    },
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use axum_extra::body::AsyncReadBody;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use eyre::{eyre, Context};
use itertools::Itertools;
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use tracing::{instrument, warn, Instrument};

use crate::{
    api::{
        self,
        schema::{
            Asset, AssetId, TimelineChunk, TimelineGroup, TimelineGroupType, TimelineRequest,
        },
        ApiResult,
    },
    app_state::SharedState,
    catalog::storage_key,
    core::storage::StorageProvider,
    model::{self, repository},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_all_assets))
        .route("/:id", get(get_asset))
        .route("/thumbnail/:id/:size/:format", get(get_thumbnail))
        .route("/file/:id", get(get_asset_file))
        .route("/timeline", get(get_timeline))
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
    // TODO 404 if not found not 503
    let read = app_state.storage.open_read_stream(&thumb_key).await?;
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

#[instrument(skip(app_state))]
async fn get_timeline(
    State(app_state): State<SharedState>,
    Query(req_body): Query<TimelineRequest>,
) -> ApiResult<Json<TimelineChunk>> {
    // ignore last_fetch for now
    let start: DateTime<Utc> = match req_body.start {
        // FIXME make start date optional, as this here can lead to assets not
        // appearing if they're "in the future"
        None => Utc::now(),
        Some(ref d) => DateTime::parse_from_rfc3339(d)
            .wrap_err("bad datetime format")?
            .into(),
    };
    let start_id = match req_body.start_id {
        Some(s) => Some(model::AssetId(s.parse().wrap_err("bad asset id")?)),
        None => None,
    };
    let results = repository::asset::get_asset_timeline_chunk(
        &app_state.pool,
        &start,
        start_id,
        req_body.max_count,
    )
    .in_current_span()
    .await?;
    let grouped_by_date: Vec<(NaiveDate, Vec<_>)> = results
        .into_iter()
        .group_by(|asset| asset.base.taken_date_local().date_naive())
        .into_iter()
        .map(|(date, group)| (date, group.collect()))
        .collect();
    let groups: Vec<TimelineGroup> = grouped_by_date
        .into_iter()
        .map(|(date, group)| TimelineGroup {
            assets: group.into_iter().map(|a| a.into()).collect(),
            ty: TimelineGroupType::Day(date.and_time(NaiveTime::default()).and_utc()),
        })
        .collect();
    Ok(Json(TimelineChunk {
        date: Utc::now(),
        changed_since_last_fetch: false,
        groups,
    }))
}

fn guess_mime_type(path: &camino::Utf8Path) -> Option<&'static str> {
    let ext = path.extension()?.to_ascii_lowercase();
    match ext.as_str() {
        "mp4" => Some("video/mp4"),
        "avif" => Some("image/avif"),
        "webp" => Some("image/webp"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        _ => {
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
