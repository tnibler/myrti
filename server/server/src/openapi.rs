use utoipa::OpenApi;
use utoipa_discover::utoipa_discover;

#[utoipa_discover(
    search_paths = [
        crate => "./server/src",
    ],
    tags((name = "mediathingy"))
)]
#[derive(OpenApi)]
pub struct ApiDoc;
