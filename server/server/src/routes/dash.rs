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
use tower::util::ServiceExt;
use tracing::Instrument;

use myrti_core::{
    catalog::storage_key,
    processing::{process_control::ProcessControl, video::dash_index},
};
use myrti_data::{interact, model, repository};

use crate::{app_state::SharedState, http_error::ApiResult, schema::FileId};

pub fn router() -> Router<SharedState> {
    Router::new().route("/{id}/{*path}", get(get_dash_file).options(get_dash_file))
}

#[derive(Debug, Clone, Deserialize)]
struct DashFilePath {
    pub id: FileId,
    pub path: String,
}

#[tracing::instrument(skip(app_state), level = "trace")]
async fn get_dash_file(
    Path(path): Path<DashFilePath>,
    State(app_state): State<SharedState>,
    request: Request<Body>,
) -> ApiResult<Response> {
    let file_id: model::FileId = path.id.try_into()?;

    let key = storage_key::dash_file(file_id, format_args!("{}", path.path));

    let storage = app_state.storage.clone();
    let fs_path = storage.local_path(&key);

    if let Some(repr_filename_concat) = path.path.strip_prefix("original/original-") {
        let (repr, rest) = match repr_filename_concat.strip_prefix("video-") {
            None => match repr_filename_concat.strip_prefix("audio-") {
                Some(rest) => (dash_index::TrackType::Audio, rest),
                None => return Err(eyre!("bad path '{}'", path.path).into()),
            },
            Some(rest) => (dash_index::TrackType::Video, rest),
        };
        // TODO: clean this mess up
        if rest != "init.mp4" {
            let segment_number: usize = {
                if rest == "init.mp4" {
                    0
                } else {
                    let rest = rest.strip_suffix(".m4s").ok_or(eyre!("bad path"))?;
                    rest.parse().context("error parsing segment number")?
                }
            };
            let filename = format!("original-{repr}-{rest}");
            let have_segment = {
                let filename = filename.clone();
                let conn = app_state.pool.get().await?;
                interact!(conn, move |conn| {
                    repository::ghi_cache::have_finalized_segment(conn, file_id, &filename)
                })
                .await??
            };

            let file_exists = tokio::fs::try_exists(&fs_path).await?;
            if !file_exists || !have_segment {
                let dash_index_path = storage.local_path(&storage_key::dash_file(
                    file_id,
                    format_args!("original/original.dash_index"),
                ));
                let index = tokio::fs::read(dash_index_path)
                    .await
                    .wrap_err("error reading dash index file")?;
                let index = dash_index::DeserializedIndex::new(index)
                    .wrap_err("error reading dash index")?;
                let do_cache_insert = match (segment_number, file_exists) {
                    // init segment (0) is ignored for cache bookkeeping
                    (0, _) => false,
                    (_, true) => {
                        let conn = app_state.pool.get().await?;
                        let filename = filename.clone();
                        let row_exists = interact!(conn, move |conn| {
                            repository::ghi_cache::update_accessed_time(conn, file_id, &filename)
                        })
                        .await??;
                        !row_exists
                    }
                    (_, false) => true,
                };
                let out_dir = storage
                    .local_path(&storage_key::dash_file(file_id, format_args!("original")))
                    .into_std_path_buf();
                if do_cache_insert {
                    let filename_copy = filename.clone();
                    let conn = app_state.pool.get().await?;
                    // TODO: handle unlikely error here. if segment is already being created, wait for it
                    // Actually, this should be a completely separate db.
                    // running transactions shouldn't block this
                    // also on startup delete stray rows with status=0
                    interact!(conn, move |conn| {
                        repository::ghi_cache::insert_pending_segment(conn, file_id, &filename_copy)
                    })
                    .await??;
                }
                let original_file_path = {
                    let conn = app_state.pool.get().await?;
                    interact!(conn, move |conn| repository::asset::get_asset_path_on_disk(
                        conn, file_id
                    ))
                    .await??
                    .path_on_disk()
                    .into_std_path_buf()
                };
                let out_dir2 = out_dir.clone();
                dash_index::write_segment(
                    &index,
                    repr,
                    segment_number,
                    &original_file_path,
                    &out_dir2,
                )
                .await
                .wrap_err("Failed to create DASH segment")?;
                if do_cache_insert {
                    let metadata = tokio::fs::metadata(out_dir.join(&filename))
                        .await
                        .wrap_err("error reading segment file metadata")?;
                    let filename_copy = filename.clone();
                    let conn = app_state.pool.get().await?;
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
        }
    }
    let serve_dir = tower_http::services::ServeFile::new(&fs_path)
        .oneshot(request)
        .in_current_span()
        .await
        .wrap_err("error serving file")?;
    Ok(serve_dir.into_response())
}
