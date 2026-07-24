use axum::{
    body::Body,
    extract::{Path, Request, State},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use eyre::{eyre, Context};
use serde::Deserialize;
use tower::ServiceExt;
use tracing::Instrument;

use myrti_core::{catalog::storage_key, core::storage::StorageProvider, model};

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{AssetId, FileId},
};

pub fn router() -> Router<SharedState> {
    Router::new().route("/:id/*path", get(get_dash_file).options(get_dash_file))
}

#[derive(Debug, Clone, Deserialize)]
struct DashFilePath {
    pub id: FileId,
    pub path: String,
}

#[tracing::instrument(fields(request = true), skip(app_state), err)]
async fn get_dash_file(
    Path(path): Path<DashFilePath>,
    State(app_state): State<SharedState>,
    request: Request<Body>,
) -> ApiResult<Response> {
    let asset_id: model::FileId = path.id.try_into()?;

    let key = storage_key::dash_file(asset_id, format_args!("{}", &path.path));

    let storage = &app_state.storage;
    let fs_path = storage
        .local_path(&key)
        .await?
        .expect("not implemented for non-local StorageProvider");

    if let Some(stripped) = path.path.strip_prefix("original/original_") {
        if !tokio::fs::try_exists(&fs_path).await? {
            let (repr, rest) = match stripped.strip_prefix("video-") {
                None => match stripped.strip_prefix("audio-") {
                    Some(rest) => ("original_audio", rest),
                    None => return Err(eyre!("bad path").into()),
                },
                Some(rest) => ("original_video", rest),
            };
            let ghi_path = storage
                .local_path(&storage_key::dash_file(
                    asset_id,
                    format_args!("original/index.ghi"),
                ))
                .await?
                .expect("not supported");
            let out_dir = storage
                .local_path(&storage_key::dash_file(asset_id, format_args!("original")))
                .await?
                .expect("not supported");
            tracing::info!(?repr, "{}, {}", &ghi_path, &rest);

            let segment_number: i32 = {
                if rest == "init.mp4" {
                    0
                } else {
                    let rest = rest.strip_suffix(".m4s").ok_or(eyre!("bad path"))?;
                    rest.parse().context("error parsing segment number")?
                }
            };

            let (_tx, mut rx) = tokio::sync::mpsc::channel(5);
            myrti_core::processing::video::gpac::create_segment(
                &ghi_path,
                &out_dir,
                &repr,
                segment_number,
                None,
                &mut rx,
            )
            .await?;
        }
    }
    // TODO (#8)
    // TODO handle non-local StorageProvider
    // TODO return correct error code for not found
    // let content_type = match &path.path {
    //     path if path.ends_with("mp4") => "video/mp4",
    //     path if path.ends_with("stream.mpd") => "application/octet-stream",
    //     _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    // };
    // let read = app_state.storage.open_read_stream(&storage_key).await?;
    // let headers = [(CONTENT_TYPE, content_type)];
    let serve_dir = tower_http::services::ServeFile::new(&fs_path)
        .oneshot(request)
        .in_current_span()
        .await
        .wrap_err("error serving file")?;
    Ok(serve_dir.into_response())
}
