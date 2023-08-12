use crate::model::repository::pool::DbPool;

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct TranscodeVideo<P: ResourcePath> {
//     pub asset_id: AssetId,
//     pub target: EncodingTarget,
//     pub output_dir: P,
//     /// relative to PackageVideo::output_dir
//     pub output_path: PathBuf,
// }
//
// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct TranscodeResult {
//     pub target: EncodingTarget,
//     pub final_size: Size,
//     pub bitrate: i64,
//     /// relative to video resource_dir
//     pub output: PathBuf,
// }
//
// pub async fn apply_transcode_video(pool: &DbPool, op: &TranscodeResult) -> Result<()> {
//     let video_represention = VideoRepresentation {
//         id: VideoRepresentationId(0),
//         asset_id: asset.base.id,
//         codec_name: codec_name(&transcode.target.codec),
//         width: transcode.final_size.width,
//         height: transcode.final_size.height,
//         bitrate: transcode.bitrate,
//         path_in_resource_dir: transcode.output.clone(),
//     };
//     let _representation_id = repository::representation::insert_video_representation(
//         pool.acquire().await?,
//         video_represention,
//     )
//     .await?;
//     Ok(())
// }
