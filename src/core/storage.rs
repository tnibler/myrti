use std::path::{Path, PathBuf};

use async_trait::async_trait;
use eyre::{Context, Result};
use tokio::io::{AsyncRead, AsyncWrite};

/// Abstraction for storing data files in any backing store.
/// This interface is basically a blob store, where every object has
/// a `key` used to store and retrieve it.
/// The `key` has to also be a valid path, so that `LocalFileStorage`
/// implementation can just use the `key` as a path without any fuss.
#[async_trait]
pub trait StorageProvider {
    type Reader: AsyncRead + Unpin;
    type Writer: AsyncWrite + Unpin;
    type CommandOutFile: CommandOutputFile;

    async fn open_read_stream(&self, key: &str) -> Result<Self::Reader>;
    async fn open_write_stream(&self, key: &str) -> Result<Self::Writer>;
    async fn exists(&self, key: &str) -> Result<bool>;
    async fn new_command_out_file(&self, key: &str) -> Result<Self::CommandOutFile>;
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
pub trait CommandOutputFile {
    fn path(&self) -> &Path;
    async fn flush_to_storage(self) -> Result<()>;
}

pub struct LocalFileStorage {
    root: PathBuf,
}

impl LocalFileStorage {
    pub fn new(root: PathBuf) -> LocalFileStorage {
        LocalFileStorage { root }
    }
}

pub struct LocalOutputFile {
    path: PathBuf,
}

#[async_trait]
impl CommandOutputFile for LocalOutputFile {
    fn path(&self) -> &Path {
        &self.path
    }

    async fn flush_to_storage(self) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl StorageProvider for LocalFileStorage {
    type Reader = tokio::fs::File;
    type Writer = tokio::fs::File;
    type CommandOutFile = LocalOutputFile;

    async fn open_read_stream(&self, key: &str) -> Result<Self::Reader> {
        tokio::fs::OpenOptions::new()
            .read(true)
            .open(self.root.join(key))
            .await
            .wrap_err("error opening file for reading")
    }

    async fn open_write_stream(&self, key: &str) -> Result<Self::Writer> {
        tokio::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(self.root.join(key))
            .await
            .wrap_err("error opening file for writing")
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        tokio::fs::try_exists(self.root.join(key))
            .await
            .wrap_err("error checking if path exists")
    }

    async fn new_command_out_file(&self, key: &str) -> Result<Self::CommandOutFile> {
        Ok(LocalOutputFile {
            path: self.root.join(key),
        })
    }
}
