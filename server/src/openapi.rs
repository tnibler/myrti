use utoipa::OpenApi;
use utoipauto::utoipauto;

#[utoipauto(paths = "./server/src")]
#[derive(OpenApi)]
#[openapi(
    tags((name = "mediathingy")),
)]
pub struct ApiDoc;
