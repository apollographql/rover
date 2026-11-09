use std::{
    collections::HashMap,
    fmt,
    fmt::Formatter,
    io,
    net::{AddrParseError, SocketAddr},
    process::{ExitStatus, Stdio},
    sync::Arc,
};

use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::TryFutureExt;
use regex::Regex;
use rover_std::{Style, infoln};
use semver::Version;
use tap::TapFallible;
use timber::Level;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
};
use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceExt};

#[cfg(test)]
use crate::command::install::{PluginLevel, PluginSource};
use crate::{
    command::{
        dev::router::config::{RouterAddress, RouterHost, RouterPort},
        install::PluginProvenance,
    },
    subtask::SubtaskHandleUnit,
    utils::{
        effect::exec::{ExecCommandConfig, ExecCommandOutput},
        lifecycle,
    },
};

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
                        "WARN" => write!(f, "{} {}", warn_prefix, message)?,
                        "ERROR" => write!(f, "{} {}", error_prefix, message)?,
                        "UNKNOWN" => write!(f, "{} {}", unknown_prefix, message)?,
                        _ => write!(f, "{} {}", unknown_prefix, message)?,
                    };
                    Ok(())
                } else {
                    write!(f, "{} {}", warn_prefix, stdout)
                }
            }
            Self::Stderr(stderr) => {
                write!(f, "{} {}", error_prefix, stderr)
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
    #[error("Router Binary exited")]
    BinaryExited(io::Result<ExitStatus>),
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(derive_getters::Getters))]
pub struct RouterBinary {
    exe: Utf8PathBuf,
    #[cfg_attr(test, getter(skip))]
    provenance: PluginProvenance,
}

impl RouterBinary {
    pub const fn new(exe: Utf8PathBuf, provenance: PluginProvenance) -> RouterBinary {
        RouterBinary { exe, provenance }
    }

    /// The exact version that was resolved (FR55) — see [`PluginProvenance::version`].
    #[allow(unused)]
    pub const fn version(&self) -> &Version {
        &self.provenance.version
    }

    pub const fn provenance(&self) -> &PluginProvenance {
        &self.provenance
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
    listen_address: RouterAddress,
    license: Option<Utf8PathBuf>,
    children: Arc<lifecycle::Children>,
}

impl<Spawn: Send> RunRouterBinary<Spawn> {
    fn args(&self) -> Vec<String> {
        let mut args = vec![
            "--supergraph".to_string(),
            self.supergraph_schema_path.to_string(),
            "--hot-reload".to_string(),
            "--config".to_string(),
            self.config_path.to_string(),
            "--log".to_string(),
            self.log_level.unwrap_or(Level::INFO).to_string(),
            "--dev".to_string(),
            "--listen".to_string(),
            self.listen_address.to_string(),
        ];
        if let Some(license) = &self.license {
            args.push("--license".to_string());
            args.push(license.to_string());
        }
        args
    }
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
        let children = Arc::clone(&self.children);
        // Held until the child's watcher is registered, so that a signal arriving while the
        // child is still being spawned waits for it instead of leaving it orphaned.
        let starting = children.starting();
        tokio::task::spawn(async move {
            let args = self.args();

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
                    // It is registered with the harness too, so that a signal that ends
                    // Rover without the command's own cleanup still stops the child.
                    let stopping = children.stopping();
                    children.spawn({
                        async move {
                            tokio::select! {
                                _ = cancellation_token.cancelled() => {
                                    let _ = child.kill().await;
                                },
                                _ = stopping.cancelled() => {
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
                    // The watcher is registered, so the harness waits for it from here on.
                    drop(starting);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, net::IpAddr};

    use camino::Utf8PathBuf;
    use semver::Version;

    use super::*;
    use crate::command::dev::router::config::{RouterAddress, RouterHost, RouterPort};

    struct MockSpawn;

    fn test_binary() -> RunRouterBinary<MockSpawn> {
        test_binary_with(MockSpawn, lifecycle::Children::new())
    }

    fn test_binary_with<Spawn: Send>(
        spawn: Spawn,
        children: Arc<lifecycle::Children>,
    ) -> RunRouterBinary<Spawn> {
        let router_address = RouterAddress::new(
            Some(RouterHost::Default(IpAddr::V4(std::net::Ipv4Addr::new(
                127, 0, 0, 1,
            )))),
            Some(RouterPort::Default(4000)),
        );
        RunRouterBinary::<Spawn>::builder()
            .router_binary(RouterBinary::new(
                Utf8PathBuf::from("/fake/path"),
                PluginProvenance::new(
                    "router",
                    Version::parse("1.0.0").unwrap(),
                    PluginSource::Installed,
                    PluginLevel::Global,
                    Utf8PathBuf::from("/fake/path"),
                ),
            ))
            .config_path(Utf8PathBuf::from("/fake/config.yaml"))
            .supergraph_schema_path(Utf8PathBuf::from("/fake/schema.graphql"))
            .spawn(spawn)
            .listen_address(router_address)
            .env(HashMap::new())
            .children(children)
            .build()
    }

    #[test]
    fn args_omit_license_when_not_set() {
        let binary = test_binary();
        assert!(!binary.args().contains(&"--license".to_string()));
    }

    #[test]
    fn args_include_license_when_set() {
        let binary = RunRouterBinary {
            license: Some(Utf8PathBuf::from("/fake/license.jwt")),
            ..test_binary()
        };
        let args = binary.args();
        let license_flag_index = args
            .iter()
            .position(|arg| arg == "--license")
            .expect("--license flag should be present");
        assert_eq!(args[license_flag_index + 1], "/fake/license.jwt");
    }

    /// A router that is running when the harness stops the process's children is killed, and
    /// is reaped before the stop returns, so it is gone by the time the process exits.
    #[cfg(unix)]
    #[tokio::test]
    async fn the_router_is_killed_when_the_harness_stops_children() {
        use std::{sync::Mutex, time::Duration};

        use tower::service_fn;

        let pid = Arc::new(Mutex::new(None));
        let spawn = service_fn({
            let pid = Arc::clone(&pid);
            move |_: ExecCommandConfig| {
                let pid = Arc::clone(&pid);
                async move {
                    let child = tokio::process::Command::new("sleep")
                        .arg("60")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()?;
                    *pid.lock().unwrap() = child.id();
                    Ok::<_, io::Error>(child)
                }
            }
        });
        let children = lifecycle::Children::new();
        let binary = test_binary_with(spawn, Arc::clone(&children));
        let (sender, _logs) = tokio::sync::mpsc::unbounded_channel();
        binary.handle(sender, None);

        // The child is still being started, so stopping must wait for it rather than return
        // at once and leave it behind.
        children.stop(Duration::from_secs(5)).await;

        let pid = pid
            .lock()
            .unwrap()
            .expect("the router should have been spawned");
        let still_running = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(!still_running, "the router child {pid} outlived the stop");
    }
}
