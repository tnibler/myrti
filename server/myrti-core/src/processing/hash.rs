use eyre::{Context, Result};
use fasthash::{SeaHasher, StreamHasher};
use std::hash::Hasher;

#[tracing::instrument(skip(s), level = "trace")]
pub async fn hash_file<R>(mut s: R) -> Result<u64>
where
    R: std::io::Read + Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel::<std::io::Result<u64>>();
    rayon::spawn(move || {
        let mut hasher: SeaHasher = Default::default();

        let res = match hasher.write_stream(&mut s) {
            Err(e) => Err(e),
            Ok(_) => Ok(hasher.finish()),
        };
        tx.send(res).unwrap();
    });
    rx.await
        .wrap_err("could not hash file")?
        .wrap_err("could not hash file")
}
