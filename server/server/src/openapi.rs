use utoipa::OpenApi;
use utoipa_discover::utoipa_discover;

#[utoipa_discover(
    search_paths = [
        crate => "./server/src",
    ],
    tags((name = "myrti"))
)]
#[derive(OpenApi)]
pub struct ApiDoc;
