use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use http::Uri;
use rover_std::{Fs, RoverStdError};
use thiserror::Error;

use self::{
    parser::{ParseRouterConfigError, RouterConfigParser},
    state::{RunRouterConfigDefault, RunRouterConfigFinal, RunRouterConfigReadConfig},
};
use crate::utils::effect::read_file::ReadFile;

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

impl Display for RouterAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let host = self
            .host
            .to_string()
            .replace("127.0.0.1", "localhost")
            .replace("0.0.0.0", "localhost")
            .replace("[::]", "localhost")
            .replace("[::1]", "localhost");
        write!(f, "http://{}:{}", host, self.port)
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
        path: Option<&Utf8PathBuf>,
    ) -> Result<RunRouterConfig<RunRouterConfigFinal>, ReadRouterConfigError> {
        match path {
            Some(path) => {
                Fs::assert_path_exists(&path).map_err(ReadRouterConfigError::Fs)?;

                match read_file_impl.read_file(&path).await {
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
                        let health_check_path = router_config.health_check_path();
                        let listen_path = router_config.listen_path();

                        Ok(RunRouterConfigFinal {
                            listen_path,
                            address,
                            health_check_enabled,
                            health_check_endpoint,
                            health_check_path,
                            raw_config: contents.to_string(),
                        })
                    }
                    Err(RoverStdError::EmptyFile { .. }) => Ok(RunRouterConfigFinal::default()),
                    Err(err) => Err(ReadRouterConfigError::ReadFile {
                        path: path.clone(),
                        source: Box::new(err),
                    }),
                }
            }
            None => Ok(RunRouterConfigFinal::default()),
        }
        .map(|state| RunRouterConfig { state })
    }
}

impl RunRouterConfig<RunRouterConfigFinal> {
    #[allow(unused)]
    pub fn listen_path(&self) -> Option<String> {
        self.state.listen_path.clone()
    }

    #[allow(unused)]
    pub fn address(&self) -> &RouterAddress {
        &self.state.address
    }

    pub fn health_check_enabled(&self) -> bool {
        self.state.health_check_enabled
    }

    pub fn health_check_endpoint(&self) -> Option<&SocketAddr> {
        self.state.health_check_endpoint.as_ref()
    }

    pub fn health_check_path(&self) -> String {
        self.state.health_check_path.clone()
    }

    pub fn raw_config(&self) -> String {
        self.state.raw_config.clone()
    }

    #[allow(unused)]
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
