use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    /// Query that should produce exactly one result did not produce any (e.g., by id)
    #[error("Expected exactly 1 query result row, got 0")]
    RowNotFound,
    /// Failed to convert db row to model type
    #[error("Unexpected combination of rows")]
    InvalidRowFields,
    #[error(transparent)]
    Other(sqlx::Error),
}

impl From<sqlx::Error> for DbError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            sqlx::Error::RowNotFound => DbError::RowNotFound,
            other => DbError::Other(other),
        }
    }
}
