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
    include: Option<Vec<String>>,
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
    pub include_globs: Vec<Glob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    pub path: PathBuf,
    pub name: Option<String>,
    pub db_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BinPaths {
    pub ffmpeg: Option<(PathBuf, Vec<String>)>,
    pub ffprobe: Option<(PathBuf, Vec<String>)>,
    pub exiftool: Option<(PathBuf, Vec<String>)>,
    pub gpac: Option<(PathBuf, Vec<String>)>,
}

impl BinPaths {
    pub fn ffmpeg_path(&self) -> (&Path, &[String]) {
        match self.ffmpeg.as_ref() {
            Some((path, args)) => (path.as_path(), args.as_slice()),
            None => ("ffmpeg".into(), &[]),
        }
    }

    pub fn ffprobe_path(&self) -> (&Path, &[String]) {
        match self.ffprobe.as_ref() {
            Some((path, args)) => (path.as_path(), args.as_slice()),
            None => ("ffprobe".into(), &[]),
        }
    }

    pub fn exiftool_path(&self) -> (&Path, &[String]) {
        match self.exiftool.as_ref() {
            Some((path, args)) => (path.as_path(), args.as_slice()),
            None => ("exiftool".into(), &[]),
        }
    }

    pub fn gpac_path(&self) -> (&Path, &[String]) {
        match self.gpac.as_ref() {
            Some((path, args)) => (path.as_path(), args.as_slice()),
            None => ("gpac".into(), &[]),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    pub asset_dirs: Vec<AssetDir>,
    pub data_dir: DataDir,
    pub bin_paths: BinPaths,
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
                        format!("error parsing exclude pattern '{s}' for asset directory {path}")
                    })
                })
                .try_collect()?;
            let include_globs = toml_value
                .include
                .into_iter()
                .flat_map(|v| v.into_iter())
                .map(|s| {
                    Glob::new(&s).wrap_err_with(|| {
                        format!("error parsing include pattern '{s}' for asset directory {path}")
                    })
                })
                .try_collect()?;
            Ok(AssetDir {
                path,
                name: toml_value.name,
                exclude_globs,
                include_globs,
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
    let bin_paths = toml_config
        .bin_paths
        .map(|bin_paths| BinPaths {
            ffmpeg: bin_paths
                .ffmpeg
                .map(PathBuf::from)
                .map(|p| (p, Default::default())),
            ffprobe: bin_paths
                .ffprobe
                .map(PathBuf::from)
                .map(|p| (p, Default::default())),
            exiftool: bin_paths
                .exiftool
                .map(PathBuf::from)
                .map(|p| (p, Default::default())),
            gpac: bin_paths
                .gpac
                .map(PathBuf::from)
                .map(|p| (p, Default::default())),
        })
        .unwrap_or_default();
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
