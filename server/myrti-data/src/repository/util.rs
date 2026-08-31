use chrono::{DateTime, TimeZone, Utc};
use eyre::{Context, Result, eyre};

use crate::model::ThumbnailType;

/// milliseconds since UNIX epoch
pub fn datetime_to_db_repr(d: &DateTime<Utc>) -> i64 {
    d.timestamp_millis()
}

/// From milliseconds since UNIX epoch
pub fn datetime_from_db_repr(unix_millis: i64) -> Result<DateTime<Utc>> {
    match Utc.timestamp_millis_opt(unix_millis) {
        chrono::LocalResult::Single(dt) => Ok(dt),
        _ => Err(eyre!(
            "error converting unix millis epoch to DateTime: {}",
            unix_millis
        )),
    }
}

pub fn hash_vec8_to_u64(v: impl AsRef<[u8]>) -> Result<u64> {
    let array: [u8; 8] = v
        .as_ref()
        .try_into()
        .wrap_err("could not parse hash from db value")?;
    Ok(u64::from_le_bytes(array))
}

pub fn hash_u64_to_vec8(u: u64) -> Vec<u8> {
    u.to_le_bytes().into_iter().collect()
}

pub fn to_db_thumbnail_type(tt: ThumbnailType) -> i32 {
    match tt {
        ThumbnailType::LargeOrigAspect => 0,
        ThumbnailType::SmallSquare => 1,
    }
}

pub fn from_db_thumbnail_type(i: i32) -> Result<ThumbnailType> {
    match i {
        0 => Ok(ThumbnailType::LargeOrigAspect),
        1 => Ok(ThumbnailType::SmallSquare),
        other => Err(eyre!("invalid db thumbnail type {}", other)),
    }
}
