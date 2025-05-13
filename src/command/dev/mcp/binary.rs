use std::collections::HashMap;
use std::fmt::Formatter;
use std::process::{ExitStatus, Stdio};
use std::{fmt, io};

use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::TryFutureExt;
use rover_std::Style;
use semver::Version;
use tap::TapFallible;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceExt};

use crate::command::dev::router::config::RouterAddress;
use crate::subtask::SubtaskHandleUnit;
use crate::utils::effect::exec::{ExecCommandConfig, ExecCommandOutput};

use super::Opts;

pub enum McpServerLog {
    Stdout(String),
    Stderr(String),
}

impl fmt::Display for McpServerLog {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let error_prefix = Style::ErrorPrefix.paint("ERROR:");
        let unknown_prefix = Style::ErrorPrefix.paint("UNKNOWN:");
        match self {
            Self::Stdout(stdout) => {
                // TODO: can we parse levels like router?
                write!(f, "{} {}", unknown_prefix, &stdout)
            }
            Self::Stderr(stderr) => {
                write!(f, "{} {}", error_prefix, &stderr)
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RunMcpServerBinaryError {
    #[error("Failed to run mcp server command: {:?}", err)]
    Spawn {
        err: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Failed to watch {descriptor} for logs")]
    OutputCapture { descriptor: String },

    #[error("Mcp Server Binary exited")]
    BinaryExited(io::Result<ExitStatus>),
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(derive_getters::Getters))]
#[allow(unused)]
pub struct McpServerBinary {
    exe: Utf8PathBuf,
    version: Version,
}

impl McpServerBinary {
    pub fn new(exe: Utf8PathBuf, version: Version) -> McpServerBinary {
        McpServerBinary { exe, version }
    }
}

#[derive(Clone, Builder)]
pub struct RunMcpServerBinary<Spawn: Send> {
    mcp_server_binary: McpServerBinary,
    supergraph_schema_path: Utf8PathBuf,
    spawn: Spawn,
    router_address: RouterAddress,
    mcp_options: Opts,
}

impl<Spawn> SubtaskHandleUnit for RunMcpServerBinary<Spawn>
where
    Spawn: Service<ExecCommandConfig, Response = Child> + Send + Clone + 'static,
    Spawn::Error: std::error::Error + Send + Sync,
    Spawn::Future: Send,
{
    type Output = Result<McpServerLog, RunMcpServerBinaryError>;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
        cancellation_token: Option<CancellationToken>,
    ) {
        let mut spawn = self.spawn.clone();
        let cancellation_token = cancellation_token.unwrap_or_default();
        tokio::task::spawn(async move {
            let mut args = vec![
                "--schema".to_string(),
                self.supergraph_schema_path.to_string(),
                "--endpoint".to_string(),
                self.router_address.to_string(),
                "--sse_port".to_string(),
                self.mcp_options.port.to_string(),
                "--introspection".to_string(),
                self.mcp_options.introspection.to_string(),
                "--allow-mutations".to_string(),
                self.mcp_options.allow_mutations.to_string(),
            ];

            if !self.mcp_options.operations.is_empty() {
                args.push("--operations".to_string());
                let mut operation_strings = self
                    .mcp_options
                    .operations
                    .into_iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<String>>();
                args.append(&mut operation_strings);
            }

            self.mcp_options.headers.into_iter().for_each(|h| {
                args.push("--headers".to_string());
                args.push(h);
            });

            // TODO: this needs auth
            if let Some(manifest) = self.mcp_options.manifest {
                args.push("--manifest".to_string());
                args.push(manifest.display().to_string());
            }

            if let Some(custom_scalars_config) = self.mcp_options.custom_scalars_config {
                args.push("--custom-scalars-config".to_string());
                args.push(custom_scalars_config.display().to_string());
            }

            let env = HashMap::from_iter([("APOLLO_ROVER".to_string(), "true".to_string())]);

            let child = spawn
                .ready()
                .and_then(|spawn| {
                    spawn.call(
                        ExecCommandConfig::builder()
                            .exe(self.mcp_server_binary.exe.clone())
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
                    let err = RunMcpServerBinaryError::Spawn { err: Box::new(err) };
                    let _ = sender
                        .send(Err(err))
                        .tap_err(|err| tracing::error!("Failed to send error message {:?}", err));
                }
                Ok(mut child) => {
                    cancellation_token
                        .run_until_cancelled(async move {
                            match child.stdout.take() {
                                Some(stdout) => {
                                    tokio::task::spawn({
                                        let sender = sender.clone();
                                        async move {
                                            let mut lines = BufReader::new(stdout).lines();
                                            while let Ok(Some(line)) =
                                                lines.next_line().await.tap_err(|err| {
                                                    tracing::error!(
                                                        "Error reading from router stdout: {:?}",
                                                        err
                                                    )
                                                })
                                            {
                                                let _ = sender
                                                    .send(Ok(McpServerLog::Stdout(line)))
                                                    .tap_err(|err| {
                                                        tracing::error!(
                                                    "Failed to send router stdout message. {:?}",
                                                    err
                                                )
                                                    });
                                            }
                                        }
                                    });
                                }
                                None => {
                                    let err = RunMcpServerBinaryError::OutputCapture {
                                        descriptor: "stdin".to_string(),
                                    };
                                    let _ = sender.send(Err(err)).tap_err(|err| {
                                        tracing::error!("Failed to send error message {:?}", err)
                                    });
                                }
                            }
                            match child.stderr.take() {
                                Some(stderr) => {
                                    tokio::task::spawn({
                                        let sender = sender.clone();
                                        async move {
                                            let mut lines = BufReader::new(stderr).lines();
                                            while let Ok(Some(line)) =
                                                lines.next_line().await.tap_err(|err| {
                                                    tracing::error!(
                                                        "Error reading from router stderr: {:?}",
                                                        err
                                                    )
                                                })
                                            {
                                                let _ = sender
                                                    .send(Ok(McpServerLog::Stderr(line)))
                                                    .tap_err(|err| {
                                                        tracing::error!(
                                                "Failed to send router stderr message. {:?}",
                                                err
                                            )
                                                    });
                                            }
                                        }
                                    });
                                }
                                None => {
                                    let err = RunMcpServerBinaryError::OutputCapture {
                                        descriptor: "stdin".to_string(),
                                    };
                                    let _ = sender.send(Err(err)).tap_err(|err| {
                                        tracing::error!("Failed to send error message {:?}", err)
                                    });
                                }
                            }
                            // Spawn a task that just sits listening to the Router binary, and if it
                            // exits, fire an error to say so, such that we can stop Rover Dev
                            // running if this happens.
                            tokio::spawn({
                                async move {
                                    let res = child.wait().await;
                                    let _ = sender
                                        .send(Err(RunMcpServerBinaryError::BinaryExited(res)))
                                        .tap_err(|err| {
                                            tracing::error!(
                                                "Failed to send router stderr message. {:?}",
                                                err
                                            )
                                        });
                                }
                            })
                            .await
                        })
                        .await;
                }
            }
        });
    }
}
