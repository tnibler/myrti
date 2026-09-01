use camino::Utf8PathBuf as PathBuf;
use eyre::{Context, Result};
use tokio::io::AsyncRead;

#[derive(thiserror::Error, Debug)]
pub enum StorageReadError {
    #[error("File with key '{0}' does not exist")]
    FileNotFound(String),
    #[error(transparent)]
    IOError {
        #[from]
        source: tokio::io::Error,
    },
    #[error(transparent)]
    Unknown {
        #[from]
        source: eyre::Report,
    },
}

#[derive(Debug, Clone)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    #[tracing::instrument(err, skip(self), level = "trace")]
    pub async fn open_read_stream(
        &self,
        key: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageReadError> {
        use tokio::io::ErrorKind;
        let open = tokio::fs::OpenOptions::new()
            .read(true)
            .open(self.root.join(key))
            .await;
        match open {
            Ok(f) => Ok(Box::new(f)),
            Err(err) => Err(match err.kind() {
                ErrorKind::NotFound => StorageReadError::FileNotFound(key.to_owned()),
                err => StorageReadError::IOError { source: err.into() },
            }),
        }
    }

    #[tracing::instrument(err, skip(self), level = "trace")]
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let path = self.root.join(key);
        tokio::fs::try_exists(&path)
            .await
            .wrap_err_with(|| format!("error checking if path {path} exists"))
    }

    pub fn local_path(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }
}
