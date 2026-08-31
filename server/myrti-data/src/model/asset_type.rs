use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Copy, Hash)]
pub enum AssetType {
    Image,
    Video,
}
