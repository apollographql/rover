use anyhow::Context;
use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use crossbeam_channel::unbounded;
use lazycell::LazyCell;
use rover_std::Fs;
use serde_json::json;

use std::{net::SocketAddr, str::FromStr};

use crate::{command::dev::do_dev::log_err_and_continue, RoverResult};

/// [`RouterConfigHandler`] is reponsible for orchestrating the YAML configuration file
/// passed to the router plugin, optionally watching a user's router configuration file for changes
#[derive(Debug, Clone)]
pub struct RouterConfigHandler {
    /// the path to a user provided router config
    input_config_path: Option<Utf8PathBuf>,

    /// the path to the router config we actually pass to the router
    tmp_config_path: Utf8PathBuf,

    /// the port provided by the CLI argument `--supergraph-port`
    port_override: Option<u16>,

    /// the address provided by the CLI argument `--supergraph-address`
    ip_override: Option<String>,

    /// the interior state of the router config, re-populated on each read of the input config
    config_state: LazyCell<RouterConfigState>,
}

impl RouterConfigHandler {
    /// Create a [`RouterConfigHandler`]
    pub fn new(
        tmp_config_path: Utf8PathBuf,
        input_config_path: Option<Utf8PathBuf>,
        port_override: Option<u16>,
        ip_override: Option<String>,
    ) -> Self {
        Self {
            tmp_config_path,
            input_config_path,
            port_override,
            ip_override,
            config_state: LazyCell::new(),
        }
    }

    pub fn refresh_state(&mut self) -> RoverResult<()> {
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
            input_yaml.insert(serde_yaml::to_value("supergraph")?, serde_yaml::to_value(
                json!({"listen": format!("{ip}:{port}", ip = ip, port = port)}),
            )?);

            // disable the health check unless they have their own config
            if input_yaml
                .get("health-check")
                .and_then(|h| h.as_mapping())
                .is_none()
            {
                input_yaml.insert(serde_yaml::to_value("health-check")?, serde_yaml::to_value(json!({"enabled": false}))?);
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

        Fs::write_file(&self.tmp_config_path, &config)?;

        let config_state = RouterConfigState::builder()
            .ip(ip)
            .port(port)
            .config(config)
            .build();

        self.config_state.replace(config_state);

        Ok(())
    }

    /// Get the path to the temp router config
    pub fn tmp_router_config_path(&self) -> Utf8PathBuf {
        self.tmp_config_path.clone()
    }

    /// Start up the router config handler
    pub fn start(&mut self) -> RoverResult<()> {
        // initialiize the tmp config
        self.refresh_state()?;

        // if a router config was passed, start watching it in the background for changes
        if let Some(input_config_path) = self.input_config_path.clone() {
            // start up channels for sending and receiving router configuration
            let (tx, rx) = unbounded();

            let watch_path = input_config_path.clone();
            rayon::spawn(move || {
                Fs::watch_file(&watch_path, tx);
            });

            loop {
                rx.recv().unwrap_or_else(|_| {
                    panic!(
                        "an unexpected error occurred while watching {}",
                        &input_config_path
                    )
                });
    
                // on each incoming change to the file, refresh the state
                let _ = self.refresh_state().map_err(log_err_and_continue);
            }

        }

        Ok(())
    }

    /// Get the current router config contents
    pub fn get_config_yaml(&mut self) -> RoverResult<String> {
        Ok(self.get_state()?.get_config())
    }

    /// Get the current router listening address
    pub fn get_router_socket_address(&mut self) -> RoverResult<SocketAddr> {
        self.get_state()?.get_socket_address()
    }

    /// Get the current state of the router config
    fn get_state(&mut self) -> RoverResult<RouterConfigState> {
        if let Some(state) = self.config_state.borrow() {
            Ok(state.clone())
        } else {
            self.refresh_state()?;
            self.get_state()
        }
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
