#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy)]
#[repr(i32)]
pub enum DbAssetType {
    Image = 1,
    Video = 2,
}
