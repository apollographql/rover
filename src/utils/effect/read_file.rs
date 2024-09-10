#[cfg(test)]
use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use camino::Utf8PathBuf;
use rover_std::{Fs, RoverStdError};

#[cfg_attr(test, mockall::automock(type Error = AnyhowError;))]
#[async_trait]
pub trait ReadFile {
    type Error: std::fmt::Debug + 'static;
    async fn read_file(&self, path: &Utf8PathBuf) -> Result<String, Self::Error>;
}

#[derive(Default)]
pub struct FsReadFile {}

#[async_trait]
impl ReadFile for FsReadFile {
    type Error = RoverStdError;
    async fn read_file(&self, path: &Utf8PathBuf) -> Result<String, Self::Error> {
        Fs::read_file(path)
    }
}
