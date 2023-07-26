use color_eyre::eyre::{bail, Context, Result};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlAssetDir {
    path: String,
    name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlDataDir {
    path: String,
    name: Option<String>,
    #[serde(rename = "maxSize")]
    max_size: Option<String>,
    #[serde(rename = "maxDiskUsage")]
    max_disk_usage: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlConfig {
    #[serde(rename = "AssetDirs")]
    pub asset_dirs: Vec<TomlAssetDir>,
    #[serde(rename = "DataDirs")]
    pub data_dirs: Vec<TomlDataDir>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssetDir {
    pub path: PathBuf,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DataDir {
    pub path: PathBuf,
    pub name: Option<String>,
    pub max_size: Option<u64>,
    pub max_disk_usage: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    pub asset_dirs: Vec<AssetDir>,
    pub data_dirs: Vec<DataDir>,
}

pub async fn read_config(path: &Path) -> Result<Config> {
    let toml_str = tokio::fs::read_to_string(path)
        .await
        .context(format!("Error reading config file {}", path.display()))?;
    let toml_config: TomlConfig = toml::from_str(&toml_str).context("Error parsing config file")?;
    let asset_dirs: Vec<AssetDir> = toml_config
        .asset_dirs
        .into_iter()
        .map(|toml_value| {
            let path = PathBuf::from_str(&toml_value.path)?;
            Ok(AssetDir {
                path,
                name: toml_value.name,
            })
        })
        .collect::<Result<_>>()?;
    let data_dirs: Vec<DataDir> = toml_config
        .data_dirs
        .into_iter()
        .map(|toml_value| {
            let path = PathBuf::from_str(&toml_value.path)?;
            let max_size: Option<u64> = toml_value
                .max_size
                .as_ref()
                .map(|s| parse_size::parse_size(s))
                .transpose()?;
            let max_disk_usage = match toml_value.max_disk_usage {
                Some(percent) if percent > 0 && percent <= 100 => Some(percent as i32),
                Some(other) => bail!(
                    "Error parsing config: invalid disk use percentage {}",
                    other
                ),
                None => None,
            };
            Ok(DataDir {
                path,
                name: toml_value.name,
                max_size,
                max_disk_usage,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Config {
        asset_dirs,
        data_dirs,
    })
}
