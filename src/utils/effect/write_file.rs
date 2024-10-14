use async_trait::async_trait;
use camino::Utf8PathBuf;
use rover_std::{Fs, RoverStdError};

#[cfg_attr(test, derive(thiserror::Error, Debug))]
#[cfg(test)]
#[cfg_attr(test, error("MockWriteFileError"))]
pub struct MockWriteFileError {}

#[cfg_attr(test, mockall::automock(type Error = MockWriteFileError;))]
#[async_trait]
pub trait WriteFile {
    type Error: std::error::Error + Send + 'static;
    async fn write_file(&self, path: &Utf8PathBuf, contents: &[u8]) -> Result<(), Self::Error>;
}

#[derive(Clone, Default)]
pub struct FsWriteFile {}

#[async_trait]
impl WriteFile for FsWriteFile {
    type Error = RoverStdError;
    async fn write_file(&self, path: &Utf8PathBuf, contents: &[u8]) -> Result<(), Self::Error> {
        Fs::write_file(path, contents)
    }
}
