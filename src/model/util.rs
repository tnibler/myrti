use chrono::{DateTime, Utc};
use eyre::{eyre, Context, Result};
use std::path::{Path, PathBuf};

pub fn opt_path_to_string(path: &Option<PathBuf>) -> Result<Option<String>> {
    match path.as_ref() {
        None => Ok(None),
        Some(p) => Ok(Some(
            p.to_str()
                .ok_or_else(|| eyre!("non unicode file path not supported"))?
                .to_string(),
        )),
    }
}

pub fn path_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    path.as_ref()
        .to_str()
        .map(|s| s.to_owned())
        .ok_or_else(|| eyre!("non unicode file path not supported"))
}

/// Formats as RFC3339 with nanoseconds
pub fn datetime_to_db_repr(d: &DateTime<Utc>) -> String {
    d.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

/// Parses from RFC3339 with nanoseconds
pub fn datetime_from_db_repr(s: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(s)
        .wrap_err("error parsing RFC3339 datetime")?
        .with_timezone(&Utc))
}
