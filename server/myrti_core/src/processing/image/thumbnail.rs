use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{Context, Result, eyre};

use crate::{
    catalog::image_conversion_target::ImageFormatTarget,
    model::Size,
    processing::{
        image::ffmpeg_snapshot::ffmpeg_snapshot, process_control::ProcessControlReceiver,
    },
};

use super::{
    OutDimension,
    vips_wrapper::{self, VipsThumbnailParams},
};

#[derive(Debug)]
pub struct ThumbnailParams {
    pub in_path: PathBuf,
    pub outputs: Vec<(PathBuf, ImageFormatTarget)>,
    pub out_dimension: OutDimension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailResult {
    pub actual_size: Size,
}

pub async fn generate_thumbnail(params: ThumbnailParams) -> Result<ThumbnailResult> {
    let tempdir = tempfile::tempdir()?;

    let vips_params = VipsThumbnailParams {
        in_path: params.in_path.clone(),
        out_dimension: params.out_dimension,
    };
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<_>>();
    rayon::spawn(move || {
        let inner = move || {
            let vips_wrapper::VipsThumbailResult { actual_size, image } =
                vips_wrapper::generate_thumbnail(vips_params)?;
            for (out_path, format) in params.outputs {
                let filename = out_path
                    .file_name()
                    .ok_or_else(|| eyre!("thumbnail output path {} has no file_name", out_path))?;
                let tmp_out: PathBuf = tempdir
                    .path()
                    .with_file_name(filename)
                    .try_into()
                    .expect("all path components were utf-8");

                vips_wrapper::save_image(&image, &tmp_out, &format)?;

                std::fs::create_dir_all(out_path.parent().expect("path is not empty"))
                    .wrap_err("error creating thumb data directory")?;
                std::fs::rename(&tmp_out, &out_path).wrap_err_with(|| {
                    eyre!(
                        "error copying temp file {} to destination {}",
                        tmp_out,
                        out_path
                    )
                })?;
            }
            Ok(actual_size)
        };
        tx.send(inner()).unwrap();
    });
    let actual_size = rx.await.unwrap().wrap_err_with(|| {
        format!(
            "error generating thumbnail with libvips (input: {})",
            params.in_path
        )
    })?;

    Ok(ThumbnailResult {
        actual_size: Size {
            width: actual_size.width,
            height: actual_size.height,
        },
    })
}

pub async fn generate_video_thumbnail(
    params: ThumbnailParams,
    control_recv: &mut ProcessControlReceiver,
) -> Result<ThumbnailResult> {
    let snapshot_path = tempfile::Builder::new()
        .prefix("snap")
        .suffix(".webp")
        .tempfile()
        .wrap_err("could not create temp file")?
        .into_temp_path();
    let utf8_snapshot_path: camino::Utf8PathBuf = snapshot_path
        .to_path_buf()
        .try_into()
        .expect("tempfile paths should be UTF8");
    // fixme ffmpeg path should come from config
    ffmpeg_snapshot(
        &params.in_path,
        &utf8_snapshot_path,
        Some("ffmpeg"),
        control_recv,
    )
    .await
    .wrap_err("error taking video snapshot")?;
    generate_thumbnail(ThumbnailParams {
        in_path: utf8_snapshot_path,
        ..params
    })
    .await
}

pub async fn generate_thumbhash(in_path: PathBuf) -> Result<String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<_>>();
    rayon::spawn(move || {
        let image = match vips_wrapper::generate_thumbnail_for_thumbhash(&in_path) {
            Ok(img) => img,
            Err(e) => {
                tx.send(Err(e)).unwrap();
                return;
            }
        };
        let thumbhash = fast_thumbhash::rgba_to_thumb_hash_b91(
            image.width.try_into().unwrap(),
            image.height.try_into().unwrap(),
            image.data(),
        );
        tx.send(Ok(thumbhash)).unwrap();
    });
    rx.await?
}
