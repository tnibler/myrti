use eyre::{eyre, Result};
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
