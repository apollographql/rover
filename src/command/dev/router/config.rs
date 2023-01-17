use anyhow::Context;
use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use crossbeam_channel::{unbounded, Receiver};
use rover_std::{Fs, Emoji};
use serde_json::json;
use tempdir::TempDir;

use std::{
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, Mutex},
};

use crate::{
    command::dev::{do_dev::log_err_and_continue, SupergraphOpts},
    RoverError, RoverResult,
};

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
            value.supergraph_address.clone(),
            value.supergraph_port.clone(),
        )
    }
}

impl RouterConfigHandler {
    /// Create a [`RouterConfigHandler`]
    pub fn new(
        input_config_path: Option<Utf8PathBuf>,
        ip_override: Option<String>,
        port_override: Option<u16>,
    ) -> RoverResult<Self> {
        let tmp_dir = TempDir::new("supergraph")?;
        let tmp_config_dir_path = Utf8PathBuf::try_from(tmp_dir.into_path())?;

        let tmp_config_path = tmp_config_dir_path.join("router.yaml");
        let tmp_composed_path = tmp_config_dir_path.join("supergraph.graphql");

        let config_reader = RouterConfigReader::new(
            input_config_path.clone(),
            ip_override.clone(),
            port_override.clone(),
        );

        let config_state = config_reader.read()?;

        Fs::write_file(&tmp_config_path, config_state.get_config())?;

        Ok(Self {
            config_reader,
            config_state: Arc::new(Mutex::new(config_state)),
            tmp_router_config_path: tmp_config_path,
            tmp_supergraph_schema_path: tmp_composed_path,
        })
    }

    /// Start up the router config handler
    pub fn start(self) -> RoverResult<()> {
        // if a router config was passed, start watching it in the background for changes

        if let Some(state_receiver) = self.config_reader.watch() {
            rayon::spawn(move || loop {
                let config_state = state_receiver
                    .recv()
                    .expect("could not watch router config");
                let _ = Fs::write_file(&self.tmp_router_config_path, config_state.get_config()).map_err(|e| log_err_and_continue(e.into()));
                eprintln!("{}successfully updated router config", Emoji::Success);
                *self
                    .config_state
                    .lock()
                    .expect("could not acquire lock on router configuration state") = config_state;
            });
        }

        Ok(())
    }

    /// If the router config handler should watch a user input router config for changes
    pub fn should_watch(&self) -> bool {
        self.config_reader.should_watch()
    }

    /// The address the router should listen on
    pub fn get_router_address(&self) -> RoverResult<SocketAddr> {
        self.config_state
            .lock()
            .expect("could not acquire lock on router config state")
            .get_socket_address()
    }

    /// Get the name of the interprocess socket address to communicate with other rover dev sessions
    pub fn get_ipc_address(&self) -> RoverResult<String> {
        let socket_name = format!("supergraph-{}.sock", self.get_router_address()?);
        {
            use interprocess::local_socket::NameTypeSupport::{self, *};
            let socket_prefix = match NameTypeSupport::query() {
                OnlyPaths | Both => "/tmp/",
                OnlyNamespaced => "@",
            };
            Ok(format!("{}{}", socket_prefix, socket_name))
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
    /// the IP address for the router to listen on
    ip: String,

    /// the port for the router to listen on
    port: String,

    /// the resolved YAML content
    config: String,
}

#[buildstructor]
impl RouterConfigState {
    #[builder]
    /// Create a new [`RouterConfigState`]
    pub fn new(ip: String, port: String, config: String) -> Self {
        Self { ip, port, config }
    }

    /// Get the socket address
    pub fn get_socket_address(&self) -> RoverResult<SocketAddr> {
        Ok(SocketAddr::from_str(&format!(
            "{ip}:{port}",
            ip = &self.ip,
            port = &self.port
        ))?)
    }

    /// Get the config contents
    pub fn get_config(&self) -> String {
        self.config.clone()
    }
}

#[derive(Debug, Clone)]
struct RouterConfigReader {
    input_config_path: Option<Utf8PathBuf>,
    ip_override: Option<String>,
    port_override: Option<u16>,
}

impl RouterConfigReader {
    pub fn new(
        input_config_path: Option<Utf8PathBuf>,
        ip_override: Option<String>,
        port_override: Option<u16>,
    ) -> Self {
        Self {
            input_config_path,
            ip_override,
            port_override,
        }
    }

    /// if the config file should be watched
    pub fn should_watch(&self) -> bool {
        self.input_config_path.is_some()
    }

    fn read(&self) -> RoverResult<RouterConfigState> {
        let default_ip = "127.0.0.1".to_string();
        let default_port = 3000;

        let (ip, port, config) = if let Some(input_config_path) = &self.input_config_path {
            let input_config_contents = Fs::read_file(input_config_path)?;
            let mut input_yaml: serde_yaml::Mapping = serde_yaml::from_str(&input_config_contents)
                .with_context(|| format!("{} is not valid YAML.", &input_config_path))?;

            let (yaml_ip, yaml_port) = if let Some(socket_addr) = input_yaml
                .get("supergraph")
                .and_then(|s| s.as_mapping())
                .and_then(|s| s.get("listen"))
                .and_then(|l| l.as_str())
            {
                let socket_addr: Vec<String> = socket_addr.split(':').map(String::from).collect();
                (socket_addr.get(0).cloned(), socket_addr.get(1).cloned())
            } else {
                (None, None)
            };

            // resolve the ip and port
            // precedence is:
            // 1) CLI option
            // 2) `supergraph.listen` in `router.yaml`
            // 3) Default of 127.0.0.1:3000
            let ip = self
                .ip_override
                .clone()
                .unwrap_or_else(|| yaml_ip.unwrap_or(default_ip));
            let port = self
                .port_override
                .map(|p| p.to_string())
                .unwrap_or_else(|| yaml_port.unwrap_or_else(|| default_port.to_string()));

            // update their yaml with the ip and port CLI options
            input_yaml.insert(
                serde_yaml::to_value("supergraph")?,
                serde_yaml::to_value(json!({
                    "listen": format!("{ip}:{port}", ip = ip, port = port)
                }))?,
            );

            // disable the health check unless they have their own config
            if input_yaml
                .get("health-check")
                .and_then(|h| h.as_mapping())
                .is_none()
            {
                input_yaml.insert(
                    serde_yaml::to_value("health-check")?,
                    serde_yaml::to_value(json!({"enabled": false}))?,
                );
            }

            let config = serde_yaml::to_string(&input_yaml)?;

            (ip, port, config)
        } else {
            let ip = self.ip_override.clone().unwrap_or(default_ip);
            let port = self.port_override.unwrap_or(default_port).to_string();
            let config = format!(
                r#"---
    supergraph:
    listen: {ip}:{port}
    health-check:
    enabled: false
                    "#,
                ip = ip,
                port = port
            );

            (ip, port, config)
        };

        Ok(RouterConfigState::builder()
            .ip(ip)
            .port(port)
            .config(config)
            .build())
    }

    pub fn watch(self) -> Option<Receiver<RouterConfigState>> {
        if let Some(input_config_path) = &self.input_config_path {
            let (raw_tx, raw_rx) = unbounded();
            let (state_tx, state_rx) = unbounded();
            Fs::watch_file(&input_config_path, raw_tx);
            rayon::spawn(move || loop {
                raw_rx
                    .recv()
                    .expect("could not watch router configuration file");
                if let Ok(results) = self.read().map_err(log_err_and_continue) {
                    state_tx
                        .send(results)
                        .expect("could not update router configuration file");
                } else {
                    eprintln!("invalid router configuration, continuing to use old config");
                }
            });
            Some(state_rx)
        } else {
            None
        }
    }
}
