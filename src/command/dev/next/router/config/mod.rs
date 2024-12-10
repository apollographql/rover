use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use http::Uri;
use rover_std::{Fs, RoverStdError};
use thiserror::Error;

use crate::utils::effect::read_file::ReadFile;

use self::{
    parser::{ParseRouterConfigError, RouterConfigParser},
    state::{RunRouterConfigDefault, RunRouterConfigFinal, RunRouterConfigReadConfig},
};

mod parser;
pub mod remote;
mod state;

const DEFAULT_ROUTER_IP_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const DEFAULT_ROUTER_PORT: u16 = 4000;

#[derive(Error, Debug)]
pub enum ReadRouterConfigError {
    #[error(transparent)]
    Fs(RoverStdError),
    #[error("Failed to read file at {}", .path)]
    ReadFile {
        path: Utf8PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("{} is not valid yaml", .path)]
    Deserialization {
        path: Utf8PathBuf,
        source: serde_yaml::Error,
    },
    #[error(transparent)]
    Parse(#[from] ParseRouterConfigError),
}

#[derive(Copy, Clone, derive_getters::Getters)]
pub struct RouterAddress {
    host: IpAddr,
    port: u16,
}

#[buildstructor]
impl RouterAddress {
    #[builder]
    pub fn new(host: Option<IpAddr>, port: Option<u16>) -> RouterAddress {
        let host = host.unwrap_or(DEFAULT_ROUTER_IP_ADDR);
        let port = port.unwrap_or(DEFAULT_ROUTER_PORT);
        RouterAddress { host, port }
    }
}

impl Default for RouterAddress {
    fn default() -> Self {
        RouterAddress {
            host: DEFAULT_ROUTER_IP_ADDR,
            port: DEFAULT_ROUTER_PORT,
        }
    }
}

impl From<RouterAddress> for SocketAddr {
    fn from(value: RouterAddress) -> Self {
        let host = value.host;
        let port = value.port;
        SocketAddr::new(host, port)
    }
}

impl From<Option<SocketAddr>> for RouterAddress {
    fn from(value: Option<SocketAddr>) -> Self {
        let host = value.map(|addr| addr.ip());
        let port = value.map(|addr| addr.port());
        RouterAddress::new(host, port)
    }
}

impl From<SocketAddr> for RouterAddress {
    fn from(value: SocketAddr) -> Self {
        let host = value.ip();
        let port = value.port();
        RouterAddress { host, port }
    }
}

pub struct RunRouterConfig<State> {
    state: State,
}

impl Default for RunRouterConfig<RunRouterConfigDefault> {
    fn default() -> Self {
        RunRouterConfig {
            state: RunRouterConfigDefault,
        }
    }
}

impl RunRouterConfig<RunRouterConfigDefault> {
    pub fn with_address(
        self,
        router_address: RouterAddress,
    ) -> RunRouterConfig<RunRouterConfigReadConfig> {
        RunRouterConfig {
            state: RunRouterConfigReadConfig { router_address },
        }
    }
}

impl RunRouterConfig<RunRouterConfigReadConfig> {
    pub async fn with_config<ReadF: ReadFile<Error = RoverStdError>>(
        self,
        read_file_impl: &ReadF,
        path: &Utf8PathBuf,
    ) -> Result<RunRouterConfig<RunRouterConfigFinal>, ReadRouterConfigError> {
        Fs::assert_path_exists(&path).map_err(ReadRouterConfigError::Fs)?;

        let state = match read_file_impl.read_file(&path).await {
            Ok(contents) => {
                let yaml = serde_yaml::from_str(&contents).map_err(|err| {
                    ReadRouterConfigError::Deserialization {
                        path: path.clone(),
                        source: err,
                    }
                })?;

                let router_config = RouterConfigParser::new(&yaml);
                let address = router_config.address()?;
                let address = address
                    .map(RouterAddress::from)
                    .unwrap_or(self.state.router_address);
                let health_check_enabled = router_config.health_check_enabled();
                let health_check_endpoint = router_config.health_check_endpoint()?;
                let listen_path = router_config.listen_path()?;

                RunRouterConfigFinal {
                    listen_path,
                    address,
                    health_check_enabled,
                    health_check_endpoint,
                    raw_config: contents.to_string(),
                }
            }
            Err(RoverStdError::EmptyFile { .. }) => {
                // TODO: assumption to check; I'm not writing to the temp file because we don't
                // need to yet, we only need to return the in-memory RunRouterConfigFinal and
                // somewhere down the line of actually running the router will we write to file
                // (that we're watching, which will emit a new router config event)
                let default_config = RunRouterConfigFinal::default();
                default_config
            }
            Err(err) => {
                return Err(ReadRouterConfigError::ReadFile {
                    path: path.clone(),
                    source: Box::new(err),
                });
            }
        };

        Ok(RunRouterConfig { state })
    }
}

impl RunRouterConfig<RunRouterConfigFinal> {
    #[allow(unused)]
    pub fn listen_path(&self) -> Option<&Uri> {
        self.state.listen_path.as_ref()
    }

    #[allow(unused)]
    pub fn address(&self) -> &RouterAddress {
        &self.state.address
    }

    pub fn health_check_enabled(&self) -> bool {
        self.state.health_check_enabled
    }

    pub fn health_check_endpoint(&self) -> &Uri {
        &self.state.health_check_endpoint
    }

    pub fn router_config(&self) -> RouterConfig {
        RouterConfig(self.state.raw_config.to_string())
    }
}

pub type RouterConfigFinal = RunRouterConfig<RunRouterConfigFinal>;

pub struct RouterConfig(String);

impl RouterConfig {
    pub fn new(s: impl Into<String>) -> RouterConfig {
        RouterConfig(s.into())
    }
}

impl RouterConfig {
    pub fn inner(&self) -> &str {
        &self.0
    }
}
