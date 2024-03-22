use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use color_eyre::eyre::{bail, Context, Result};
use serde::Deserialize;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlAssetDir {
    path: String,
    name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlDataDir {
    path: String,
    name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlBinPaths {
    pub mpd_generator: Option<String>,
    pub shaka_packager: Option<String>,
    pub ffmpeg: Option<String>,
    pub ffprobe: Option<String>,
    pub exiftool: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TomlConfig {
    #[serde(rename = "AssetDirs")]
    pub asset_dirs: Vec<TomlAssetDir>,
    #[serde(rename = "DataDir")]
    pub data_dir: TomlDataDir,
    #[serde(rename = "BinPaths")]
    pub bin_paths: Option<TomlBinPaths>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDir {
    pub path: PathBuf,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    pub path: PathBuf,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinPaths {
    pub mpd_generator: Option<PathBuf>,
    pub shaka_packager: Option<PathBuf>,
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub exiftool: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub asset_dirs: Vec<AssetDir>,
    pub data_dir: DataDir,
    pub bin_paths: Option<BinPaths>,
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
            Ok(AssetDir {
                path,
                name: toml_value.name,
            })
        })
        .collect::<Result<_>>()?;
    let data_dir: DataDir = {
        let path = toml_config.data_dir.path.into();
        DataDir {
            path,
            name: toml_config.data_dir.name,
        }
    };
    let bin_paths = toml_config.bin_paths.map(|bin_paths| BinPaths {
        mpd_generator: bin_paths.mpd_generator.map(PathBuf::from),
        shaka_packager: bin_paths.shaka_packager.map(PathBuf::from),
        ffmpeg: bin_paths.ffmpeg.map(PathBuf::from),
        ffprobe: bin_paths.ffprobe.map(PathBuf::from),
        exiftool: bin_paths.exiftool.map(PathBuf::from),
    });
    Ok(Config {
        asset_dirs,
        data_dir,
        bin_paths,
    })
}
