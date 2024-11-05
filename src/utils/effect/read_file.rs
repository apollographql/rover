use async_trait::async_trait;
use camino::Utf8PathBuf;
use rover_std::{Fs, RoverStdError};

#[cfg_attr(test, derive(thiserror::Error, Debug))]
#[cfg(test)]
#[cfg_attr(test, error("MockReadFileError"))]
pub struct MockReadFileError {}

#[cfg_attr(test, mockall::automock(type Error = MockReadFileError;))]
#[async_trait]
pub trait ReadFile {
    type Error: std::error::Error + Send + 'static;
    async fn read_file(&self, path: &Utf8PathBuf) -> Result<String, Self::Error>;
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct FsReadFile {}

#[async_trait]
impl ReadFile for FsReadFile {
    type Error = RoverStdError;
    async fn read_file(&self, path: &Utf8PathBuf) -> Result<String, Self::Error> {
        Fs::read_file(path)
    }
}
