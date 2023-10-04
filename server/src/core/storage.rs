use async_trait::async_trait;
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use enum_dispatch::enum_dispatch;
use eyre::{Context, Result};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{instrument, Instrument};

/// Abstraction for storing data files in any backing store.
/// This interface is basically a blob store, where every object has
/// a `key` used to store and retrieve it.
/// In practice, the `key` is not opaque, as it is used in path like manner
/// for easier interop with DASH tools.
#[async_trait]
#[enum_dispatch(Storage)]
pub trait StorageProvider: Clone {
    async fn open_read_stream(
        &self,
        key: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageReadError>;
    async fn open_write_stream(&self, key: &str) -> Result<Box<dyn AsyncWrite + Send + Unpin>>;
    async fn exists(&self, key: &str) -> Result<bool>;
    async fn new_command_out_file(&self, key: &str) -> Result<CommandOutputFile>;
    /// If this `StorageProvider` is backed by a local filesystem,
    /// this returns the path `key` maps to assuming `key` exists.
    /// If `key` doesn't exist or the `StorageProvider` is not local,
    /// returns None.
    async fn local_path(&self, key: &str) -> Result<Option<PathBuf>>;
}

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

/// External commands (like ffmpeg) expect a local file path to write their output to,
/// which breaks the `StorageProvider` abstraction.
/// To solve this, `StorageProvider`s have to offer a local file
/// that can be written to by external commands and then "flushed" to the store
/// underlying the StorageProvider (uploaded to S3 for example).
/// Notably, this allows `StorageProviders` where objects are local files
/// to just offer the path where they choose to store the object
/// to commands directly.
#[async_trait]
#[enum_dispatch(CommandOutputFile)]
pub trait StorageCommandOutput {
    fn path(&self) -> &Path;
    async fn size(&self) -> Result<u64>;
    async fn flush_to_storage(self) -> Result<()>;
}

#[enum_dispatch]
pub enum Storage {
    LocalFileStorage,
}

impl Clone for Storage {
    fn clone(&self) -> Self {
        match self {
            Self::LocalFileStorage(a) => Self::LocalFileStorage(a.clone()),
        }
    }
}

#[enum_dispatch]
pub enum CommandOutputFile {
    LocalOutputFile,
}

#[derive(Debug, Clone)]
pub struct LocalFileStorage {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LocalOutputFile {
    path: PathBuf,
    debug_only_flushed_to_storage: bool,
}

impl LocalFileStorage {
    pub fn new(root: PathBuf) -> LocalFileStorage {
        LocalFileStorage { root }
    }
}

#[async_trait]
impl StorageCommandOutput for LocalOutputFile {
    fn path(&self) -> &Path {
        &self.path
    }

    async fn size(&self) -> Result<u64> {
        let file_meta = tokio::fs::metadata(&self.path)
            .await
            .wrap_err("error getting file metadata")?;
        Ok(file_meta.len())
    }

    #[instrument(skip(self), level = "debug")]
    async fn flush_to_storage(mut self) -> Result<()> {
        self.debug_only_flushed_to_storage = true;
        Ok(())
    }
}

impl Drop for LocalOutputFile {
    fn drop(&mut self) {
        // debug_assert!(
        //     self.debug_only_flushed_to_storage,
        //     "forgot to call write_to_storage!"
        // );
    }
}

#[async_trait]
impl StorageProvider for LocalFileStorage {
    #[instrument(skip(self), level = "debug")]
    async fn open_read_stream(
        &self,
        key: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageReadError> {
        use tokio::io::ErrorKind;
        let open = tokio::fs::OpenOptions::new()
            .read(true)
            .open(self.root.join(key))
            .in_current_span()
            .await;
        match open {
            Ok(f) => Ok(Box::new(f)),
            Err(err) => Err(match err.kind() {
                ErrorKind::NotFound => StorageReadError::FileNotFound(key.to_owned()),
                err => StorageReadError::IOError { source: err.into() },
            }),
        }
    }

    #[instrument(skip(self), level = "debug")]
    async fn open_write_stream(&self, key: &str) -> Result<Box<dyn AsyncWrite + Send + Unpin>> {
        Ok(Box::new(
            tokio::fs::OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(self.root.join(key))
                .await
                .wrap_err("error opening file for writing")?,
        ))
    }

    #[instrument(skip(self), level = "debug")]
    async fn exists(&self, key: &str) -> Result<bool> {
        tokio::fs::try_exists(self.root.join(key))
            .await
            .wrap_err("error checking if path exists")
    }

    #[instrument(skip(self), level = "debug")]
    async fn new_command_out_file(&self, key: &str) -> Result<CommandOutputFile> {
        let path = self.root.join(key);
        if let Some(parent) = &path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .wrap_err("could not create directory")?;
        }
        Ok(LocalOutputFile {
            path,
            debug_only_flushed_to_storage: false,
        }
        .into())
    }

    async fn local_path(&self, key: &str) -> Result<Option<PathBuf>> {
        Ok(Some(self.root.join(key)))
    }
}
