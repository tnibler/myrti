use crate::http_error::HttpError;

pub mod routes;
pub mod schema;

pub type ApiResult<T> = Result<T, HttpError>;
