use std::path::PathBuf;

use eyre::Result;

use crate::core::storage::{CommandOutputFile, StorageCommandOutput};

use super::{
    vips_wrapper::{self, VipsThumbnailParams},
    OutDimension,
};

pub struct ThumbnailParams<'a> {
    pub in_path: PathBuf,
    pub outputs: Vec<&'a CommandOutputFile>,
    pub out_dimension: OutDimension,
}

pub trait GenerateThumbnailTrait {
    fn generate_thumbnail(params: ThumbnailParams) -> Result<()>;
}

pub struct GenerateThumbnail {}

pub struct GenerateThumbnailMock {}

impl GenerateThumbnailTrait for GenerateThumbnail {
    fn generate_thumbnail(params: ThumbnailParams) -> Result<()> {
        let out_paths: Vec<PathBuf> = params
            .outputs
            .iter()
            .map(|f| f.path().to_path_buf())
            .collect();
        let vips_params = VipsThumbnailParams {
            in_path: params.in_path,
            out_paths,
            out_dimension: params.out_dimension,
        };
        vips_wrapper::generate_thumbnail(vips_params)
    }
}

impl GenerateThumbnailTrait for GenerateThumbnailMock {
    fn generate_thumbnail(params: ThumbnailParams) -> Result<()> {
        Ok(())
    }
}
