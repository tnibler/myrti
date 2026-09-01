use std::os::unix::fs::MetadataExt;

use axum::{
    Router,
    body::Body,
    extract::{Path, Request, State},
    response::{IntoResponse, Response},
    routing::get,
};
use eyre::{Context, eyre};
use serde::Deserialize;
use tower::ServiceExt;
use tracing::Instrument;

use myrti_core::catalog::storage_key;
use myrti_data::{interact, model, repository};

use crate::{app_state::SharedState, http_error::ApiResult, schema::FileId};

pub fn router() -> Router<SharedState> {
    Router::new().route("/:id/*path", get(get_dash_file).options(get_dash_file))
}

#[derive(Debug, Clone, Deserialize)]
struct DashFilePath {
    pub id: FileId,
    pub path: String,
}

#[tracing::instrument(fields(request = true), skip(app_state), err, level = "trace")]
async fn get_dash_file(
    Path(path): Path<DashFilePath>,
    State(app_state): State<SharedState>,
    request: Request<Body>,
) -> ApiResult<Response> {
    let file_id: model::FileId = path.id.try_into()?;

    let key = storage_key::dash_file(file_id, format_args!("{}", path.path));

    let storage = &app_state.storage;
    let fs_path = storage.local_path(&key);

    if let Some(repr_filename_concat) = path.path.strip_prefix("original/original_") {
        let (repr, rest) = match repr_filename_concat.strip_prefix("video-") {
            None => match repr_filename_concat.strip_prefix("audio-") {
                Some(rest) => ("original_audio", rest),
                None => return Err(eyre!("bad path '{}'", path.path).into()),
            },
            Some(rest) => ("original_video", rest),
        };
        let segment_number: i32 = {
            if rest == "init.mp4" {
                0
            } else {
                let rest = rest.strip_suffix(".m4s").ok_or(eyre!("bad path"))?;
                rest.parse().context("error parsing segment number")?
            }
        };
        let filename = format!("{repr}-{rest}");
        let conn = app_state.pool.get().await?;
        let file_exists = tokio::fs::try_exists(&fs_path).await?;
        let do_cache_insert = match (segment_number, file_exists) {
            // init segment (0) is ignored for cache bookkeeping
            (0, _) => false,
            (_, true) => {
                let filename = filename.clone();
                let row_exists = interact!(conn, move |conn| {
                    repository::ghi_cache::update_accessed_time(conn, file_id, &filename)
                })
                .await??;
                !row_exists
            }
            (_, false) => true,
        };
        let out_dir =
            storage.local_path(&storage_key::dash_file(file_id, format_args!("original")));
        if do_cache_insert {
            let filename_copy = filename.clone();
            // TODO: handle unlikely error here. if segment is already being created, wait for it
            interact!(conn, move |conn| {
                repository::ghi_cache::insert_pending_segment(conn, file_id, &filename_copy)
            })
            .await??;
        }
        if !file_exists {
            let (_tx, mut rx) = tokio::sync::mpsc::channel(5);
            let ghi_path = storage.local_path(&storage_key::dash_file(
                file_id,
                format_args!("original/index.ghi"),
            ));

            myrti_core::processing::video::gpac::create_segment(
                &ghi_path,
                &out_dir,
                repr,
                segment_number,
                None,
                &mut rx,
            )
            .await?;
        }
        if do_cache_insert {
            if file_exists {
                tracing::debug!(path = ?fs_path, "GHI segment file exists, but no row in GhiSegmentCache");
            }
            let metadata = tokio::fs::metadata(out_dir.join(&filename))
                .await
                .wrap_err("error reading segment file metadata")?;
            let filename_copy = filename.clone();
            interact!(conn, move |conn| {
                repository::ghi_cache::finalize_segment(
                    conn,
                    file_id,
                    &filename_copy,
                    metadata.size().try_into().expect("fits in i64"),
                )
            })
            .await??;
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
