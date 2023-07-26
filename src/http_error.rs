use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use eyre;

pub struct HttpError(eyre::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Server error: {}", self.0),
        )
            .into_response()
    }
}

macro_rules! impl_from {
    ($from:ty) => {
        impl From<$from> for HttpError {
            fn from(err: $from) -> Self {
                Self(err.into())
            }
        }
    };
}

impl_from!(std::io::Error);
impl_from!(color_eyre::Report);
