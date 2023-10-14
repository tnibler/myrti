use eyre::Result;

use crate::model::{AlbumId, AssetId};

use super::pool::DbPool;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CreateAlbum {
    pub name: String,
    pub description: Option<String>,
}

pub async fn create_album(create_album: &CreateAlbum, pool: &DbPool) -> Result<AlbumId> {
    let result = sqlx::query!(
        r#"
INSERT INTO Album(id, name, description, created_at, changed_at)
VALUES
(NULL, ?, ?, ?, ?);
    "#
    );
}

