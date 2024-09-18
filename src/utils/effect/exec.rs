use std::process::Output;

#[cfg(test)]
use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use camino::Utf8PathBuf;
use tokio::process::Command;

#[cfg_attr(test, mockall::automock(type Error = AnyhowError;))]
#[async_trait]
pub trait ExecCommand {
    type Error: std::fmt::Debug + 'static;
    async fn exec_command<'a>(
        &self,
        path: &Utf8PathBuf,
        args: &[String],
    ) -> Result<Output, Self::Error>;
}

#[derive(Default)]
pub struct TokioCommand {}

#[async_trait]
impl ExecCommand for TokioCommand {
    type Error = std::io::Error;
    async fn exec_command<'a>(
        &self,
        path: &Utf8PathBuf,
        args: &[String],
    ) -> Result<Output, Self::Error> {
        Command::new(path).args(args).output().await
    }
}
