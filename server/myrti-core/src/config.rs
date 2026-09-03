use std::str::FromStr;

use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{Context, Result};
use globset::Glob;
use itertools::Itertools;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlAssetDir {
    path: String,
    name: Option<String>,
    exclude: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlDataDir {
    path: String,
    name: Option<String>,
    db_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlBinPaths {
    pub ffmpeg: Option<String>,
    pub ffprobe: Option<String>,
    pub exiftool: Option<String>,
    pub gpac: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlConfig {
    #[serde(rename = "AssetDirs")]
    pub asset_dirs: Vec<TomlAssetDir>,
    #[serde(rename = "DataDir")]
    pub data_dir: TomlDataDir,
    #[serde(rename = "BinPaths")]
    pub bin_paths: Option<TomlBinPaths>,
    pub address: Option<String>,
    pub port: Option<u16>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AssetDir {
    pub path: PathBuf,
    pub name: Option<String>,
    pub exclude_globs: Vec<Glob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    pub path: PathBuf,
    pub name: Option<String>,
    pub db_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BinPaths {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub exiftool: Option<PathBuf>,
    pub gpac: Option<PathBuf>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    pub asset_dirs: Vec<AssetDir>,
    pub data_dir: DataDir,
    pub bin_paths: Option<BinPaths>,
    pub address: Option<String>,
    pub port: Option<u16>,
}

pub async fn read_config(path: &Path) -> Result<Config> {
    let toml_str = tokio::fs::read_to_string(path)
        .await
        .context(format!("Error reading config file {}", path))?;
    let toml_config: TomlConfig = toml::from_str(&toml_str).context("Error parsing config file")?;
    let asset_dirs: Vec<AssetDir> = toml_config
        .asset_dirs
        .into_iter()
        .map(|toml_value| {
            let path = PathBuf::from_str(&toml_value.path)?;
            let exclude_globs = toml_value
                .exclude
                .into_iter()
                .flat_map(|v| v.into_iter())
                .map(|s| {
                    Glob::new(&s).wrap_err_with(|| {
                        format!("error parsing exclude pattern for asset directory {}", path)
                    })
                })
                .try_collect()?;
            Ok(AssetDir {
                path,
                name: toml_value.name,
                exclude_globs,
            })
        })
        .collect::<Result<_>>()?;
    let data_dir: DataDir = {
        let path = toml_config.data_dir.path.into();
        DataDir {
            path,
            name: toml_config.data_dir.name,
            db_path: toml_config.data_dir.db_path.map(PathBuf::from),
        }
    };
    let bin_paths = toml_config.bin_paths.map(|bin_paths| BinPaths {
        ffmpeg: bin_paths.ffmpeg.map(PathBuf::from),
        ffprobe: bin_paths.ffprobe.map(PathBuf::from),
        exiftool: bin_paths.exiftool.map(PathBuf::from),
        gpac: bin_paths.gpac.map(PathBuf::from),
    });
    let address = toml_config.address;
    let port: Option<u16> = toml_config.port;
    Ok(Config {
        asset_dirs,
        data_dir,
        bin_paths,
        address,
        port,
    })
}
