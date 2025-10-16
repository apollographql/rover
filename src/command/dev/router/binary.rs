use std::collections::HashMap;
use std::fmt::Formatter;
use std::net::{AddrParseError, SocketAddr};
use std::process::{ExitStatus, Stdio};
use std::{fmt, io};

use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::TryFutureExt;
use regex::Regex;
use rover_std::{Style, infoln};
use semver::Version;
use tap::TapFallible;
use timber::Level;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceExt};

use super::hot_reload::HotReloadError;
use crate::RoverError;
use crate::command::dev::router::config::{RouterAddress, RouterHost, RouterPort};
use crate::subtask::SubtaskHandleUnit;
use crate::utils::effect::exec::{ExecCommandConfig, ExecCommandOutput};

pub enum RouterLog {
    Stdout(String),
    Stderr(String),
}

fn should_select_log_message(log_message: &str) -> bool {
    // For most info-level messages, we want to pipe them to tracing only.
    // However, a few info-level messages (e.g. for confirming that the router started up)
    // we want to pluck them so we can treat them differently.
    //
    // the match "exposed at http" captures expressions:
    // * Health check exposed at http://127.0.0.1:8088/health
    // * GraphQL endpoint exposed at http://127.0.0.1:4090/
    log_message.matches("exposed at http").next().is_some()
}

fn produce_special_message(raw_message: &str) {
    let starting_message_regex = Regex::new(r"^.*\s+.*://(.*:[0-9]+).*\s+.*").unwrap();

    let contents = match starting_message_regex.captures(raw_message) {
        None => raw_message.to_string(),
        Some(captures) => {
            let socket_address: Option<Result<SocketAddr, AddrParseError>> =
                captures.get(1).map(|m| m.as_str().parse());
            match socket_address {
                Some(Ok(socket_addr)) => {
                    let router_address = RouterAddress::new(
                        Some(RouterHost::CliOption(socket_addr.ip())),
                        Some(RouterPort::CliOption(socket_addr.port())),
                    )
                    .pretty_string();
                    format!(
                        "Your supergraph is running! head to {router_address} to query your supergraph"
                    )
                }
                _ => raw_message.to_string(),
            }
        }
    };
    infoln!("{}", contents)
}

impl fmt::Display for RouterLog {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let warn_prefix = Style::WarningPrefix.paint("WARN:");
        let error_prefix = Style::ErrorPrefix.paint("ERROR:");
        let unknown_prefix = Style::ErrorPrefix.paint("UNKNOWN:");
        match self {
            Self::Stdout(stdout) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(stdout) {
                    let fields = &parsed["fields"];
                    let level = parsed["level"].as_str().unwrap_or("UNKNOWN");
                    let message = fields["message"]
                        .as_str()
                        .or_else(|| {
                            // Message is in a slightly different location depending on the
                            // version of Router
                            parsed["message"].as_str()
                        })
                        .unwrap_or(stdout);

                    match level {
                        "INFO" if should_select_log_message(message) => {
                            produce_special_message(message);
                        }
                        "INFO" => tracing::info!(%message),
                        "DEBUG" => tracing::debug!(%message),
                        "TRACE" => tracing::trace!(%message),
                        "WARN" => write!(f, "{} {}", warn_prefix, &message)?,
                        "ERROR" => write!(f, "{} {}", error_prefix, &message)?,
                        "UNKNOWN" => write!(f, "{} {}", unknown_prefix, &message)?,
                        _ => write!(f, "{} {}", unknown_prefix, &message)?,
                    };
                    Ok(())
                } else {
                    write!(f, "{} {}", warn_prefix, &stdout)
                }
            }
            Self::Stderr(stderr) => {
                write!(f, "{} {}", error_prefix, &stderr)
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RunRouterBinaryError {
    #[error("Service failed to come into a ready state: {:?}", .err)]
    ServiceReadyError {
        err: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to run router command: {:?}", err)]
    Spawn {
        err: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to watch {descriptor} for logs")]
    OutputCapture { descriptor: String },
    #[error("Failed healthcheck for router")]
    HealthCheckFailed,
    #[error("Something went wrong with an internal dependency, {}: {}", .dependency, .err)]
    Internal { dependency: String, err: String },
    #[error("Failed to write file to path: {}. {}", .path, .err)]
    WriteFileError {
        path: Utf8PathBuf,
        err: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to parse config: {}.", .err)]
    Config {
        err: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to expand config: {}.", .err)]
    Expansion { err: RoverError },
    #[error("Router Binary exited")]
    BinaryExited(io::Result<ExitStatus>),
}

impl From<HotReloadError> for RunRouterBinaryError {
    fn from(value: HotReloadError) -> Self {
        match value {
            HotReloadError::Config { err } => Self::Config { err },
            HotReloadError::Expansion { err } => Self::Expansion { err },
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(derive_getters::Getters))]
pub struct RouterBinary {
    exe: Utf8PathBuf,
    #[allow(unused)]
    version: Version,
}

impl RouterBinary {
    pub const fn new(exe: Utf8PathBuf, version: Version) -> RouterBinary {
        RouterBinary { exe, version }
    }
}

#[derive(Clone, Builder)]
pub struct RunRouterBinary<Spawn: Send> {
    router_binary: RouterBinary,
    config_path: Utf8PathBuf,
    supergraph_schema_path: Utf8PathBuf,
    spawn: Spawn,
    log_level: Option<Level>,
    env: HashMap<String, String>,
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
        cancellation_token: Option<CancellationToken>,
    ) {
        let mut spawn = self.spawn.clone();
        let cancellation_token = cancellation_token.unwrap_or_default();
        tokio::task::spawn(async move {
            let args = vec![
                "--supergraph".to_string(),
                self.supergraph_schema_path.to_string(),
                "--hot-reload".to_string(),
                "--config".to_string(),
                self.config_path.to_string(),
                "--log".to_string(),
                self.log_level.unwrap_or(Level::INFO).to_string(),
                "--dev".to_string(),
            ];

            let child = spawn
                .ready()
                .and_then(|spawn| {
                    spawn.call(
                        ExecCommandConfig::builder()
                            .exe(self.router_binary.exe.clone())
                            .args(args)
                            .env(self.env)
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
                Ok(mut child) => {
                    if let Some(stdout) = child.stdout.take() {
                        tokio::task::spawn({
                            let sender = sender.clone();
                            async move {
                                let mut lines = BufReader::new(stdout).lines();
                                while let Ok(Some(line)) = lines.next_line().await.tap_err(|err| {
                                    tracing::error!("Error reading from router stdout: {:?}", err)
                                }) {
                                    let _ =
                                        sender.send(Ok(RouterLog::Stdout(line))).tap_err(|err| {
                                            tracing::error!(
                                                "Failed to send router stdout message. {:?}",
                                                err
                                            )
                                        });
                                }
                            }
                        });
                    } else {
                        let err = RunRouterBinaryError::OutputCapture {
                            descriptor: "stdin".to_string(),
                        };
                        let _ = sender.send(Err(err)).tap_err(|err| {
                            tracing::error!("Failed to send error message {:?}", err)
                        });
                    }

                    if let Some(stderr) = child.stderr.take() {
                        tokio::task::spawn({
                            let sender = sender.clone();
                            async move {
                                let mut lines = BufReader::new(stderr).lines();
                                while let Ok(Some(line)) = lines.next_line().await.tap_err(|err| {
                                    tracing::error!("Error reading from router stderr: {:?}", err)
                                }) {
                                    let _ =
                                        sender.send(Ok(RouterLog::Stderr(line))).tap_err(|err| {
                                            tracing::error!(
                                                "Failed to send router stderr message. {:?}",
                                                err
                                            )
                                        });
                                }
                            }
                        });
                    } else {
                        let err = RunRouterBinaryError::OutputCapture {
                            descriptor: "stdin".to_string(),
                        };
                        let _ = sender.send(Err(err)).tap_err(|err| {
                            tracing::error!("Failed to send error message {:?}", err)
                        });
                    }

                    // Spawn a task that just sits listening to the Router binary, and if it
                    // exits, fire an error to say so, such that we can stop Rover Dev
                    // running if this happens.
                    tokio::spawn({
                        async move {
                            tokio::select! {
                                _ = cancellation_token.cancelled() => {
                                    let _ = child.kill().await;
                                },
                                res = child.wait() => {
                                    let _ = sender
                                        .send(Err(RunRouterBinaryError::BinaryExited(res)))
                                        .tap_err(|err| {
                                            tracing::error!(
                                                "Failed to send router stderr message. {:?}",
                                                err
                                            )
                                        });
                                }
                            }
                        }
                    });
                }
            }
        });
    }
}
