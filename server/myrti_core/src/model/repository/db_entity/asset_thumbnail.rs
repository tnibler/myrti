use diesel::{Queryable, QueryableByName, Selectable};
use eyre::eyre;

use crate::model::{
    AssetThumbnail, AssetThumbnailId, FileId, Size, ThumbnailFormat, util::from_db_thumbnail_type,
};

#[derive(Debug, Clone, PartialEq, Eq, Queryable, QueryableByName, Selectable)]
#[diesel(table_name = super::super::schema::AssetThumbnail)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAssetThumbnail {
    pub thumbnail_id: i64,
    pub file_id: i64,
    pub ty: i32,
    pub format_name: String,
    pub width: i32,
    pub height: i32,
}

impl TryFrom<&DbAssetThumbnail> for AssetThumbnail {
    type Error = eyre::Report;

    fn try_from(value: &DbAssetThumbnail) -> Result<Self, Self::Error> {
        let format: ThumbnailFormat = match value.format_name.as_str() {
            "webp" => ThumbnailFormat::Webp,
            "avif" => ThumbnailFormat::Avif,
            other => {
                return Err(eyre!("Unknown thumbnail format from db: {}", other));
            }
        };
        Ok(AssetThumbnail {
            id: AssetThumbnailId(value.thumbnail_id),
            file_id: FileId(value.file_id),
            ty: from_db_thumbnail_type(value.ty)?,
            size: Size {
                width: value.width,
                height: value.height,
            },
            format,
        })
    }
}

impl TryFrom<DbAssetThumbnail> for AssetThumbnail {
    type Error = eyre::Report;

    fn try_from(value: DbAssetThumbnail) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
