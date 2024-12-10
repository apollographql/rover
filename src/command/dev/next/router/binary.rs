use std::{collections::HashMap, process::Stdio};

use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::TryFutureExt;
use semver::Version;
use tap::TapFallible;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
};
use tower::{Service, ServiceExt};

use crate::{
    subtask::SubtaskHandleUnit,
    utils::effect::exec::{ExecCommandConfig, ExecCommandOutput},
};

use super::config::remote::RemoteRouterConfig;

pub enum RouterLog {
    Stdout(String),
    Stderr(String),
}

#[derive(thiserror::Error, Debug)]
pub enum RunRouterBinaryError {
    #[error("Failed to run router command: {:?}", err)]
    Spawn {
        err: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to watch {descriptor} for logs")]
    OutputCapture { descriptor: String },
    #[error("Failed healthcheck for router")]
    HealthCheckFailed,
    #[error("Something went wrong with an internal dependency, {}: {}", .dependency, .error)]
    Internal { dependency: String, error: String },
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(derive_getters::Getters))]
pub struct RouterBinary {
    exe: Utf8PathBuf,
    version: Version,
}

impl RouterBinary {
    pub fn new(exe: Utf8PathBuf, version: Version) -> RouterBinary {
        RouterBinary { exe, version }
    }
}

#[derive(Clone, Builder)]
pub struct RunRouterBinary<Spawn: Send> {
    router_binary: RouterBinary,
    config_path: Utf8PathBuf,
    supergraph_schema_path: Utf8PathBuf,
    remote_config: Option<RemoteRouterConfig>,
    spawn: Spawn,
}

impl<Spawn> SubtaskHandleUnit for RunRouterBinary<Spawn>
where
    Spawn: Service<ExecCommandConfig, Response = Child> + Send + Clone + 'static,
    Spawn::Error: std::error::Error + Send + Sync,
    Spawn::Future: Send,
{
    type Output = Result<RouterLog, RunRouterBinaryError>;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
    ) -> tokio::task::AbortHandle {
        let mut spawn = self.spawn.clone();
        let remote_config = self.remote_config.clone();
        tokio::task::spawn(async move {
            let args = vec![
                "--supergraph".to_string(),
                self.supergraph_schema_path.to_string(),
                "--hot-reload".to_string(),
                "--config".to_string(),
                self.config_path.to_string(),
                "--log".to_string(),
                "info".to_string(),
                "--dev".to_string(),
            ];
            let mut env = HashMap::from_iter([("APOLLO_ROVER".to_string(), "true".to_string())]);
            if let Some(graph_ref) = remote_config.as_ref().map(|c| c.graph_ref().to_string()) {
                env.insert("APOLLO_GRAPH_REF".to_string(), graph_ref);
            }
            if let Some(api_key) = remote_config.and_then(|c| c.api_key().clone()) {
                env.insert("APOLLO_KEY".to_string(), api_key);
            }
            let child = spawn
                .ready()
                .and_then(|spawn| {
                    spawn.call(
                        ExecCommandConfig::builder()
                            .exe(self.router_binary.exe.clone())
                            .args(args)
                            .env(env)
                            .output(
                                ExecCommandOutput::builder()
                                    .stdin(Stdio::null())
                                    .stdout(Stdio::piped())
                                    .stderr(Stdio::piped())
                                    .build(),
                            )
                            .build(),
                    )
                })
                .await;
            match child {
                Err(err) => {
                    let err = RunRouterBinaryError::Spawn { err: Box::new(err) };
                    let _ = sender
                        .send(Err(err))
                        .tap_err(|err| tracing::error!("Failed to send error message {:?}", err));
                }
                Ok(mut child) => match child.stdout.take() {
                    Some(stdout) => {
                        tokio::task::spawn(async move {
                            let mut lines = BufReader::new(stdout).lines();
                            while let Ok(Some(line)) = lines.next_line().await.tap_err(|err| {
                                tracing::error!("Error reading from router stdout: {:?}", err)
                            }) {
                                let _ = sender.send(Ok(RouterLog::Stdout(line))).tap_err(|err| {
                                    tracing::error!(
                                        "Failed to send router stdout message. {:?}",
                                        err
                                    )
                                });
                            }
                        });
                    }
                    None => {
                        let err = RunRouterBinaryError::OutputCapture {
                            descriptor: "stdin".to_string(),
                        };
                        let _ = sender.send(Err(err)).tap_err(|err| {
                            tracing::error!("Failed to send error message {:?}", err)
                        });
                    }
                },
            }
        })
        .abort_handle()
    }
}
