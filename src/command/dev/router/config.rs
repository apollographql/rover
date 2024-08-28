use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Context};
use camino::Utf8PathBuf;
use crossbeam_channel::{unbounded, Receiver};
use serde_json::json;

use rover_std::Fs;

use crate::utils::expansion::expand;
use crate::{
    command::dev::{do_dev::log_err_and_continue, SupergraphOpts},
    RoverError, RoverResult,
};

const DEFAULT_ROUTER_SOCKET_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 4000);

/// [`RouterConfigHandler`] is reponsible for orchestrating the YAML configuration file
/// passed to the router plugin, optionally watching a user's router configuration file for changes
#[derive(Debug, Clone)]
pub struct RouterConfigHandler {
    /// the router configuration reader
    config_reader: RouterConfigReader,

    /// the temp path to write the patched router config out to
    tmp_router_config_path: Utf8PathBuf,

    /// the temp path to write the composed schema out to
    tmp_supergraph_schema_path: Utf8PathBuf,

    /// the current state of the router config
    config_state: Arc<Mutex<RouterConfigState>>,
}

impl TryFrom<&SupergraphOpts> for RouterConfigHandler {
    type Error = RoverError;
    fn try_from(value: &SupergraphOpts) -> Result<Self, Self::Error> {
        Self::new(
            value.router_config_path.clone(),
            value.supergraph_address,
            value.supergraph_port,
        )
    }
}

impl RouterConfigHandler {
    /// Create a [`RouterConfigHandler`]
    pub fn new(
        input_config_path: Option<Utf8PathBuf>,
        ip_override: Option<IpAddr>,
        port_override: Option<u16>,
    ) -> RoverResult<Self> {
        let tmp_dir = tempfile::Builder::new().prefix("supergraph").tempdir()?;
        let tmp_config_dir_path = Utf8PathBuf::try_from(tmp_dir.into_path())?;

        let tmp_router_config_path = tmp_config_dir_path.join("router.yaml");
        let tmp_supergraph_schema_path = tmp_config_dir_path.join("supergraph.graphql");

        let config_reader = RouterConfigReader::new(input_config_path, ip_override, port_override);

        let config_state = config_reader.read()?;

        Fs::write_file(&tmp_router_config_path, &config_state.config)?;

        Ok(Self {
            config_reader,
            config_state: Arc::new(Mutex::new(config_state)),
            tmp_router_config_path,
            tmp_supergraph_schema_path,
        })
    }

    /// Start up the router config handler
    pub fn start(self) -> RoverResult<()> {
        // if a router config was passed, start watching it in the background for changes

        if let Some(state_receiver) = self.config_reader.watch() {
            // Build a Rayon Thread pool
            let tp = rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .thread_name(|idx| format!("router-config-{idx}"))
                .build()
                .map_err(|err| {
                    RoverError::new(anyhow!("could not create router config thread pool: {err}",))
                })?;
            tp.spawn(move || loop {
                let config_state = state_receiver
                    .recv()
                    .expect("could not watch router config");
                let _ = Fs::write_file(&self.tmp_router_config_path, &config_state.config)
                    .map_err(|e| log_err_and_continue(e.into()));
                eprintln!("successfully updated router config");
                *self
                    .config_state
                    .lock()
                    .expect("could not acquire lock on router configuration state") = config_state;
            });
        }

        Ok(())
    }

    /// The address the router should listen on
    pub fn get_router_address(&self) -> SocketAddr {
        self.config_state
            .lock()
            .expect("could not acquire lock on router config state")
            .socket_addr
            .unwrap_or(DEFAULT_ROUTER_SOCKET_ADDR)
    }

    /// The path the router should listen on
    pub fn get_router_listen_path(&self) -> String {
        self.config_state
            .lock()
            .expect("could not acquire lock on router config state")
            .listen_path
            .clone()
    }

    /// Get the name of the interprocess socket address to communicate with other rover dev sessions
    pub fn get_raw_socket_name(&self) -> String {
        let socket_name = format!("supergraph-{}.sock", self.get_router_address());
        #[cfg(windows)]
        {
            format!("\\\\.\\pipe\\{}", socket_name)
        }
        #[cfg(unix)]
        {
            format!("/tmp/{}", socket_name)
        }
    }

    /// The path to the composed supergraph schema
    pub fn get_supergraph_schema_path(&self) -> Utf8PathBuf {
        self.tmp_supergraph_schema_path.clone()
    }

    /// The path to the patched router config YAML
    pub fn get_router_config_path(&self) -> Utf8PathBuf {
        self.tmp_router_config_path.clone()
    }
}

#[derive(Debug, Clone)]
pub struct RouterConfigState {
    /// Where the router should listen
    pub socket_addr: Option<SocketAddr>,

    /// the resolved YAML content
    pub config: String,

    /// the path the router is listening on
    pub listen_path: String,
}

#[derive(Debug, Clone)]
struct RouterConfigReader {
    input_config_path: Option<Utf8PathBuf>,
    ip_override: Option<IpAddr>,
    port_override: Option<u16>,
}

impl RouterConfigReader {
    pub fn new(
        input_config_path: Option<Utf8PathBuf>,
        ip_override: Option<IpAddr>,
        port_override: Option<u16>,
    ) -> Self {
        Self {
            input_config_path,
            ip_override,
            port_override,
        }
    }

    fn read(&self) -> RoverResult<RouterConfigState> {
        let mut yaml = self
            .input_config_path
            .as_ref()
            .and_then(|path| {
                Fs::assert_path_exists(path).ok().map(|_| {
                    let input_config_contents = Fs::read_file(path)?;
                    serde_yaml::from_str(&input_config_contents)
                        .with_context(|| format!("{} is not valid YAML.", path))
                        .map_err(RoverError::from)
                        .and_then(expand)
                        .and_then(|value| match value {
                            serde_yaml::Value::Mapping(mapping) => Ok(mapping),
                            _ => Err(anyhow!("Router config should be a YAML mapping").into()),
                        })
                })
            })
            .transpose()?
            .unwrap_or_default();

        let yaml_socket_addr = yaml
            .get("supergraph")
            .and_then(|s| s.get("listen"))
            .and_then(|l| l.as_str())
            .and_then(|s| s.parse::<SocketAddr>().ok());

        // resolve the ip and port
        // precedence is:
        // 1) CLI option
        // 2) `supergraph.listen` in `router.yaml`
        // 3) Nothing—use router's defaults
        let socket_addr = match (&self.ip_override, &self.port_override, yaml_socket_addr) {
            (Some(ip), Some(port), _) => Some(SocketAddr::new(*ip, *port)),
            (Some(ip), None, yaml) => {
                let mut socket_addr = yaml.unwrap_or(DEFAULT_ROUTER_SOCKET_ADDR);
                socket_addr.set_ip(*ip);
                Some(socket_addr)
            }
            (None, Some(port), yaml) => {
                let mut socket_addr = yaml.unwrap_or(DEFAULT_ROUTER_SOCKET_ADDR);
                socket_addr.set_port(*port);
                Some(socket_addr)
            }
            (None, None, Some(yaml)) => Some(yaml),
            (None, None, None) => None,
        };

        if let Some(socket_addr) = socket_addr {
            // update YAML with the ip and port CLI options
            yaml.entry("supergraph".into())
                .or_insert_with(|| serde_yaml::Mapping::new().into())
                .as_mapping_mut()
                .ok_or_else(|| anyhow!("`supergraph` key in router YAML must be a mapping"))?
                .insert("listen".into(), serde_yaml::to_value(socket_addr)?);
        }

        // disable the health check unless they have their own config
        if yaml
            .get("health_check")
            .or_else(|| yaml.get("health-check"))
            .and_then(|h| h.as_mapping())
            .is_none()
        {
            yaml.insert(
                serde_yaml::to_value("health_check")?,
                serde_yaml::to_value(json!({"enabled": false}))?,
            );
        }
        let listen_path = yaml
            .get("supergraph")
            .and_then(|s| s.as_mapping())
            .and_then(|l| l.get("path"))
            .and_then(|p| p.as_str())
            .unwrap_or_default()
            .to_string();

        let yaml_string = serde_yaml::to_string(&yaml)?;

        if let Some(path) = &self.input_config_path {
            if Fs::assert_path_exists(path).is_err() {
                eprintln!("{path} does not exist, creating a router config from CLI options.");
                Fs::write_file(path, &yaml_string)?;
            }
        }

        Ok(RouterConfigState {
            socket_addr,
            config: yaml_string,
            listen_path,
        })
    }

    pub fn watch(self) -> Option<Receiver<RouterConfigState>> {
        if let Some(input_config_path) = &self.input_config_path {
            let (raw_tx, mut raw_rx) = tokio::sync::mpsc::unbounded_channel();
            let (state_tx, state_rx) = unbounded();
            Fs::watch_file(input_config_path, raw_tx);
            tokio::spawn(async move {
                loop {
                    raw_rx
                        .recv()
                        .await
                        .expect("could not watch router configuration file")
                        .expect("could not watch router configuration file");
                    if let Ok(results) = self.read().map_err(log_err_and_continue) {
                        state_tx
                            .send(results)
                            .expect("could not update router configuration file");
                    } else {
                        eprintln!("invalid router configuration, continuing to use old config");
                    }
                }
            });
            Some(state_rx)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use rstest::rstest;

    use crate::command::dev::router::RouterConfigHandler;

    #[rstest]
    #[cfg_attr(windows, case("\\\\.\\pipe\\supergraph-127.0.0.1:4000.sock"))]
    #[cfg_attr(unix, case("/tmp/supergraph-127.0.0.1:4000.sock"))]
    fn test_socket_types_correctly_detected(#[case] expected_ipc_address: String) {
        let ip_addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port_number = 4000;
        let r_config = RouterConfigHandler::new(None, Some(ip_addr), Some(port_number))
            .expect("failed to create config handler");
        assert_eq!(
            r_config.get_raw_socket_name(),
            format!("{}", expected_ipc_address)
        );
    }
}
