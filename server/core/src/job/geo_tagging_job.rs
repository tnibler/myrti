use camino::Utf8PathBuf as PathBuf;
use eyre::{eyre, Context, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    core::job::{Job, JobHandle, JobProgress, JobResultType},
    model::{
        repository::{self, db::DbPool},
        AssetId,
    },
};

pub struct GeoTaggingJobConfig {
    pub reverse_geocoder_db_path: PathBuf,
}

pub struct GeoTaggingJob {
    params: GeoTaggingParams,
    pool: DbPool,
    config: GeoTaggingJobConfig,
}

#[derive(Debug)]
pub struct GeoTaggingJobResult {
    pub failed: Vec<FailedGeoTagging>,
}

#[derive(Debug)]
pub struct FailedGeoTagging {
    pub asset_id: AssetId,
    pub err: eyre::Report,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoTaggingParams {
    pub asset_ids: Vec<AssetId>,
}

impl GeoTaggingJob {
    pub fn new(
        params: GeoTaggingParams,
        pool: DbPool,
        config: GeoTaggingJobConfig,
    ) -> GeoTaggingJob {
        GeoTaggingJob {
            params,
            pool,
            config,
        }
    }

    pub async fn start(
        self,
        status_tx: mpsc::Sender<JobProgress>,
        cancel: CancellationToken,
    ) -> JobHandle {
        // TODO send progress updates
        status_tx
            .send(JobProgress {
                percent: None,
                description: "".to_string(),
            })
            .await
            .unwrap();
        let (tx, rx) = mpsc::channel::<JobProgress>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();

        let join_handle =
            tokio::task::spawn(
                async move { JobResultType::Geotagging(self.run(cancel_copy).await) },
            );
        JobHandle {
            progress_rx: rx,
            join_handle,
            cancel,
        }
    }

    async fn run(self, cancel: CancellationToken) -> Result<GeoTaggingJobResult> {
        // let rgc = geocode::ReverseGeocoder::new(&self.config.reverse_geocoder_db_path)
        //     .await
        //     .wrap_err("error initializing ReverseGeocoder")?;
        let mut failed: Vec<FailedGeoTagging> = Vec::default();
        // for asset_id in &self.params.asset_ids {
        //     if cancel.is_cancelled() {
        //         return Ok(GeoTaggingJobResult { failed });
        //     }
        // let result = self.process_asset(*asset_id, &rgc).await;
        // match result {
        //     Ok(()) => {}
        //     Err(err) => {
        //         failed.push(FailedGeoTagging {
        //             asset_id: *asset_id,
        //             err,
        //         });
        //     }
        // }
        // }
        Ok(GeoTaggingJobResult { failed })
    }

    // async fn process_asset(&self, asset_id: AssetId, rgc: &ReverseGeocoder) -> Result<()> {
    //     let asset = repository::asset::get_asset(&self.pool, asset_id).await?;
    //     let gps_coords = match asset.base.gps_coordinates {
    //         None => return Ok(()),
    //         Some(c) => c,
    //     };
    //     let float_coords = geocode::Coordinates {
    //         lat: gps_coords.lat as f32 / 10e8_f32,
    //         lon: gps_coords.lon as f32 / 10e8_f32,
    //     };
    //     let lookup_result = rgc.lookup(float_coords).await;
    //     match lookup_result {
    //         Ok(geotag) => geotag,
    //         Err(err) => match err {
    //             ReverseGeocodeError::Other(err) => {
    //                 return Err(err.wrap_err("error in ReverseGeocoder"));
    //             }
    //             ReverseGeocodeError::BaseDataNotPresent => {
    //                 rgc.download_base_data().await?;
    //                 rgc.lookup(float_coords)
    //                     .await
    //                     .wrap_err("error in ReverseGeocoder")?
    //             }
    //             ReverseGeocodeError::CountryDataNotPresent { country_id } => {
    //                 rgc.download_country_data(country_id).await?;
    //                 rgc.lookup(float_coords)
    //                     .await
    //                     .wrap_err("error in ReverseGeocoder")?
    //             }
    //         },
    //     };
    //     Ok(())
    // }
}
