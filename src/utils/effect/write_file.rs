use std::pin::Pin;

use async_trait::async_trait;
use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::Future;
use rover_std::{Fs, RoverStdError};
use tower::Service;

#[cfg_attr(test, derive(thiserror::Error, Debug))]
#[cfg(test)]
#[cfg_attr(test, error("MockWriteFileError"))]
pub struct MockWriteFileError {}

#[cfg_attr(test, mockall::automock(type Error = MockWriteFileError;))]
#[async_trait]
pub trait WriteFile {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn write_file(&self, path: &Utf8PathBuf, contents: &[u8]) -> Result<(), Self::Error>;
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct FsWriteFile {}

#[async_trait]
impl WriteFile for FsWriteFile {
    type Error = RoverStdError;
    async fn write_file(&self, path: &Utf8PathBuf, contents: &[u8]) -> Result<(), Self::Error> {
        Fs::write_file(path, contents)
    }
}

#[derive(Builder)]
pub struct WriteFileRequest {
    path: Utf8PathBuf,
    contents: Option<Vec<u8>>,
}

impl Service<WriteFileRequest> for FsWriteFile {
    type Response = ();
    type Error = RoverStdError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: WriteFileRequest) -> Self::Future {
        let fut = async { Fs::write_file(req.path, req.contents.unwrap_or_default()) };
        Box::pin(fut)
    }
}
