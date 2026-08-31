use axum::{
    Router,
    extract::State,
    http::header::CONTENT_TYPE,
    response::{IntoResponse, Response},
    routing::get,
};

use myrti_data::{interact, repository};

use crate::{app_state::SharedState, http_error::ApiResult};

pub fn router() -> Router<SharedState> {
    Router::new().route("/assetPoints", get(get_all_assets_geojson))
}

#[utoipa::path(get, path = "/api/map/assetPoints", responses((status=200, body=String)))]
async fn get_all_assets_geojson(State(app_state): State<SharedState>) -> ApiResult<Response> {
    let conn = app_state.pool.get().await?;
    let geojson = interact!(conn, move |conn| repository::asset::get_all_assets_geojson(
        conn
    ))
    .await??;
    let headers = [(CONTENT_TYPE, "application/geo+json")];
    Ok((headers, geojson).into_response())
}
