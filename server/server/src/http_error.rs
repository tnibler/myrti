use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use eyre;

#[derive(Debug)]
pub struct HttpError(StatusCode, eyre::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let HttpError(status, error) = self;

        (status, format!("{:?}", error)).into_response()
    }
}

macro_rules! impl_from {
    ($from:ty) => {
        impl From<$from> for HttpError {
            fn from(err: $from) -> Self {
                Self(StatusCode::INTERNAL_SERVER_ERROR, err.into())
            }
        }
    };
}

impl_from!(std::io::Error);
impl_from!(eyre::Error);

pub type ApiResult<T> = Result<T, HttpError>;

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.1)
    }
}

pub trait HttpErrorExt {
    fn into_404(self) -> HttpError;
    fn into_400(self) -> HttpError;
    fn into_503(self) -> HttpError;
}

impl HttpErrorExt for eyre::Error {
    fn into_404(self) -> HttpError {
        HttpError(StatusCode::NOT_FOUND, self)
    }

    fn into_400(self) -> HttpError {
        HttpError(StatusCode::BAD_REQUEST, self)
    }

    fn into_503(self) -> HttpError {
        HttpError(StatusCode::INTERNAL_SERVER_ERROR, self)
    }
}
