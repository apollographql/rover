use std::{
    collections::HashMap,
    fmt,
    fmt::Formatter,
    io,
    process::{ExitStatus, Stdio},
};

use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::TryFutureExt;
use rover_std::Style;
use semver::Version;
use tap::TapFallible;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
};
use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceExt};

use crate::{
    command::dev::router::config::RouterAddress,
    subtask::SubtaskHandleUnit,
    utils::effect::exec::{ExecCommandConfig, ExecCommandOutput},
};

pub enum McpServerLog {
    Stdout(String),
    Stderr(String),
}

impl fmt::Display for McpServerLog {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdout(stdout) => {
                // TODO: add a JSON output option to the MCP Server so we can parse it
                write!(f, "{}", &stdout)
            }
            Self::Stderr(stderr) => {
                write!(f, "{} {}", Style::ErrorPrefix.paint("ERROR:"), &stderr)
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

    #[error("MCP Server Binary exited")]
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
    pub const fn new(exe: Utf8PathBuf, version: Version) -> McpServerBinary {
        McpServerBinary { exe, version }
    }
}

#[derive(Clone, Builder)]
pub struct RunMcpServerBinary<Spawn: Send> {
    mcp_server_binary: McpServerBinary,
    supergraph_schema_path: Utf8PathBuf,
    spawn: Spawn,
    router_address: RouterAddress,
    router_url_path: Option<String>,
    mcp_config_path: Option<Utf8PathBuf>,
    env: HashMap<String, String>,
}

impl<Spawn: Send> RunMcpServerBinary<Spawn> {
    // Gather the rover-specific configuration options into environment variables
    // understood by the MCP server.
    // TODO: Magic strings are not fun to debug later.
    fn opts_into_env(self) -> HashMap<String, String> {
        // Build the endpoint URL with the optional path from router config
        let endpoint = if let Some(path) = &self.router_url_path {
            format!("{}{}", self.router_address.pretty_string(), path)
        } else {
            self.router_address.pretty_string()
        };

        let overlaid = HashMap::from([
            // Configure the schema to be a local file
            ("APOLLO_MCP_SCHEMA__SOURCE".to_string(), "local".to_string()),
            (
                "APOLLO_MCP_SCHEMA__PATH".to_string(),
                self.supergraph_schema_path.to_string(),
            ),
            // Configure the endpoint from the running router instance
            ("APOLLO_MCP_ENDPOINT".to_string(), endpoint),
            (
                "APOLLO_MCP_TRANSPORT__TYPE".to_string(),
                "streamable_http".to_string(),
            ),
        ]);

        // We don't want the user's env possibly conflicting with what rover dev has configured,
        // so we overlay rover's configuration over the user's env.
        self.env.into_iter().chain(overlaid).collect()
    }
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
            let child = spawn
                .ready()
                .and_then(|spawn| {
                    spawn.call(
                        ExecCommandConfig::builder()
                            .exe(self.mcp_server_binary.exe.clone())
                            .args(
                                self.mcp_config_path
                                    .iter()
                                    .map(Utf8PathBuf::to_string)
                                    .collect(),
                            )
                            .env(self.opts_into_env())
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
                    if let Some(stdout) = child.stdout.take() {
                        tokio::task::spawn({
                            let sender = sender.clone();
                            async move {
                                let mut lines = BufReader::new(stdout).lines();
                                while let Ok(Some(line)) = lines.next_line().await.tap_err(|err| {
                                    tracing::error!(
                                        "Error reading from MCP Server stdout: {:?}",
                                        err
                                    )
                                }) {
                                    let _ = sender.send(Ok(McpServerLog::Stdout(line))).tap_err(
                                        |err| {
                                            tracing::error!(
                                                "Failed to send MCP Server stdout message. {:?}",
                                                err
                                            )
                                        },
                                    );
                                }
                            }
                        });
                    } else {
                        let err = RunMcpServerBinaryError::OutputCapture {
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
                                    tracing::error!(
                                        "Error reading from MCP Server stderr: {:?}",
                                        err
                                    )
                                }) {
                                    let _ = sender.send(Ok(McpServerLog::Stderr(line))).tap_err(
                                        |err| {
                                            tracing::error!(
                                                "Failed to send MCP Server stderr message. {:?}",
                                                err
                                            )
                                        },
                                    );
                                }
                            }
                        });
                    } else {
                        let err = RunMcpServerBinaryError::OutputCapture {
                            descriptor: "stdin".to_string(),
                        };
                        let _ = sender.send(Err(err)).tap_err(|err| {
                            tracing::error!("Failed to send error message {:?}", err)
                        });
                    }

                    // Spawn a task that just sits listening to the MCP Server binary, and if it
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
                                        .send(Err(RunMcpServerBinaryError::BinaryExited(res)))
                                        .tap_err(|err| {
                                            tracing::error!(
                                                "Failed to send MCP server stderr message. {:?}",
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, net::IpAddr};

    use camino::Utf8PathBuf;
    use semver::Version;

    use super::*;
    use crate::command::dev::router::config::{RouterAddress, RouterHost, RouterPort};

    struct MockSpawn;

    #[test]
    fn test_mcp_endpoint_without_router_url_path() {
        let router_address = RouterAddress::new(
            Some(RouterHost::Default(IpAddr::V4(std::net::Ipv4Addr::new(
                127, 0, 0, 1,
            )))),
            Some(RouterPort::Default(4000)),
        );

        let binary = McpServerBinary::new(
            Utf8PathBuf::from("/fake/path"),
            Version::parse("1.0.0").unwrap(),
        );

        let router_url_path: Option<String> = None;
        let mcp_config_path: Option<Utf8PathBuf> = None;

        let runner = RunMcpServerBinary::<MockSpawn>::builder()
            .mcp_server_binary(binary)
            .supergraph_schema_path(Utf8PathBuf::from("/fake/schema.graphql"))
            .spawn(MockSpawn)
            .router_address(router_address)
            .and_router_url_path(router_url_path)
            .and_mcp_config_path(mcp_config_path)
            .env(HashMap::new())
            .build();

        let env = runner.opts_into_env();
        let endpoint = env.get("APOLLO_MCP_ENDPOINT").unwrap();

        assert_eq!(endpoint, "http://localhost:4000");
    }

    #[test]
    fn test_mcp_endpoint_with_router_url_path() {
        let router_address = RouterAddress::new(
            Some(RouterHost::Default(IpAddr::V4(std::net::Ipv4Addr::new(
                127, 0, 0, 1,
            )))),
            Some(RouterPort::Default(4000)),
        );

        let binary = McpServerBinary::new(
            Utf8PathBuf::from("/fake/path"),
            Version::parse("1.0.0").unwrap(),
        );

        let mcp_config_path: Option<Utf8PathBuf> = None;

        let runner = RunMcpServerBinary::<MockSpawn>::builder()
            .mcp_server_binary(binary)
            .supergraph_schema_path(Utf8PathBuf::from("/fake/schema.graphql"))
            .spawn(MockSpawn)
            .router_address(router_address)
            .and_router_url_path(Some("/graphql".to_string()))
            .and_mcp_config_path(mcp_config_path)
            .env(HashMap::new())
            .build();

        let env = runner.opts_into_env();
        let endpoint = env.get("APOLLO_MCP_ENDPOINT").unwrap();

        assert_eq!(endpoint, "http://localhost:4000/graphql");
    }

    #[test]
    fn test_mcp_endpoint_with_custom_path() {
        let router_address = RouterAddress::new(
            Some(RouterHost::Default(IpAddr::V4(std::net::Ipv4Addr::new(
                127, 0, 0, 1,
            )))),
            Some(RouterPort::Default(4000)),
        );

        let binary = McpServerBinary::new(
            Utf8PathBuf::from("/fake/path"),
            Version::parse("1.0.0").unwrap(),
        );

        let mcp_config_path: Option<Utf8PathBuf> = None;

        let runner = RunMcpServerBinary::<MockSpawn>::builder()
            .mcp_server_binary(binary)
            .supergraph_schema_path(Utf8PathBuf::from("/fake/schema.graphql"))
            .spawn(MockSpawn)
            .router_address(router_address)
            .and_router_url_path(Some("/custom-path".to_string()))
            .and_mcp_config_path(mcp_config_path)
            .env(HashMap::new())
            .build();

        let env = runner.opts_into_env();
        let endpoint = env.get("APOLLO_MCP_ENDPOINT").unwrap();

        assert_eq!(endpoint, "http://localhost:4000/custom-path");
    }
}
