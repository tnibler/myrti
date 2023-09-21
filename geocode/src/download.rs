use std::path::Path;

use color_eyre::{eyre::eyre, eyre::Context, Result};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

const GEONAMES_BASE: &str = "https://download.geonames.org/export/dump";

pub async fn download_file(geoname_path: &str, out_path: &Path) -> Result<()> {
    let mut dl_stream = reqwest::get(format!("{}/{}", GEONAMES_BASE, geoname_path))
        .await
        .wrap_err("error requesting file")?
        .bytes_stream();
    let out_file = tokio::fs::File::options()
        .write(true)
        .create_new(true)
        .open(out_path)
        .await
        .wrap_err("could not open destination file")?;
    let mut out_buf = tokio::io::BufWriter::new(out_file);
    while let Some(bytes) = dl_stream.next().await {
        tokio::io::copy(&mut bytes.unwrap().as_ref(), &mut out_buf)
            .await
            .wrap_err("error writing to destination file")?;
    }
    Ok(())
}

pub async fn download_zipped_file(
    geoname_path: &str,
    file_to_extract: &str,
    out_file: &Path,
) -> Result<()> {
    let dl_out_file =
        tokio::fs::File::from_std(tempfile::tempfile().wrap_err("error creating temp file")?);
    let mut dl_stream = reqwest::get(format!("{}/{}", GEONAMES_BASE, geoname_path))
        .await
        .wrap_err("error requesting file")?
        .bytes_stream();
    let mut out_buf = tokio::io::BufWriter::new(dl_out_file);
    while let Some(bytes) = dl_stream.next().await {
        tokio::io::copy(&mut bytes.unwrap().as_ref(), &mut out_buf)
            .await
            .wrap_err("error writing to destination file")?;
    }
    out_buf.flush().await?;
    let dl_out_file = out_buf.into_inner().into_std().await;

    let file_to_extract = file_to_extract.to_owned();
    let out_file = out_file.to_owned();
    let _: Result<()> = tokio::task::spawn_blocking(move || {
        let mut zipfile = zip::ZipArchive::new(dl_out_file).wrap_err("error reading zip file")?;
        let mut extract_file = match zipfile.by_name(&file_to_extract) {
            Ok(file) => Ok(file),
            Err(_) => Err(eyre!("file not found in zip file")),
        }?;
        let extract_out_file = std::fs::File::options()
            .create_new(true)
            .write(true)
            .open(&out_file)
            .wrap_err("error creating output file")?;
        let mut out_buf = std::io::BufWriter::new(extract_out_file);
        std::io::copy(&mut extract_file, &mut out_buf)?;
        Ok(())
    })
    .await?;
    Ok(())
}
