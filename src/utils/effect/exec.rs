use std::{
    collections::HashMap,
    pin::Pin,
    process::{Output, Stdio},
};

use async_trait::async_trait;
use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::Future;
use tokio::process::{Child, Command};
use tower::Service;

#[derive(Builder, Debug)]
#[cfg_attr(test, derive(derive_getters::Getters))]
pub struct ExecCommandConfig {
    exe: Utf8PathBuf,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    output: Option<ExecCommandOutput>,
}

#[derive(Builder, Default, Debug)]
pub struct ExecCommandOutput {
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
    stdin: Option<Stdio>,
}

#[cfg_attr(test, derive(thiserror::Error, Debug))]
#[cfg(test)]
#[cfg_attr(test, error("MockExecError"))]
pub struct MockExecError {}

#[cfg_attr(test, mockall::automock(type Error = MockExecError;))]
#[async_trait]
pub trait ExecCommand {
    type Error: std::fmt::Debug + 'static;
    async fn exec_command<'a>(&self, config: ExecCommandConfig) -> Result<Output, Self::Error>;
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct TokioCommand {}

#[async_trait]
impl ExecCommand for TokioCommand {
    type Error = std::io::Error;
    async fn exec_command<'a>(&self, config: ExecCommandConfig) -> Result<Output, Self::Error> {
        let mut command = Command::new(config.exe.clone());
        let command = build_command(&mut command, config);

        command
            .spawn()
            .map_err(|err| std::io::Error::other(err.to_string()))?
            .wait_with_output()
            .await
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct TokioSpawn {}

impl Service<ExecCommandConfig> for TokioSpawn {
    type Response = Child;
    type Error = std::io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: ExecCommandConfig) -> Self::Future {
        Box::pin(async move {
            let mut command = Command::new(req.exe.clone());
            let command = build_command(&mut command, req);
            command.spawn()
        })
    }
}

fn build_command(command: &mut Command, config: ExecCommandConfig) -> &mut Command {
    let command = command
        .args(config.args.unwrap_or_default())
        .envs(config.env.unwrap_or_default());
    let output = config.output.unwrap_or_default();
    let command = if let Some(stdout) = output.stdout {
        command.stdout(stdout)
    } else {
        command
    };
    let command = if let Some(stderr) = output.stderr {
        command.stderr(stderr)
    } else {
        command
    };
    if let Some(stdin) = output.stdin {
        command.stdin(stdin)
    } else {
        command
    }
}
