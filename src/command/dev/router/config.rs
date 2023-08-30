use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::{anyhow, Context};
use camino::Utf8PathBuf;
use futures::prelude::*;
use serde_json::json;
use tempdir::TempDir;

use rover_std::{Emoji, Fs};

use crate::{
    command::dev::{event::Event, SupergraphOpts},
    utils::expansion::expand,
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
        let tmp_dir = TempDir::new("supergraph")?;
        let tmp_config_dir_path = Utf8PathBuf::try_from(tmp_dir.into_path())?;

        let tmp_router_config_path = tmp_config_dir_path.join("router.yaml");
        let tmp_supergraph_schema_path = tmp_config_dir_path.join("supergraph.graphql");

        let config_reader = RouterConfigReader::new(input_config_path, ip_override, port_override);

        Ok(Self {
            config_reader,
            tmp_router_config_path,
            tmp_supergraph_schema_path,
        })
    }

    /// Update the mirrored temp files with the new state
    pub fn write_router_config_to_tmp(&self, config_state: &RouterConfigState) -> RoverResult<()> {
        Fs::write_file(&self.tmp_router_config_path, config_state.config)?;
        eprintln!("{}successfully updated router config", Emoji::Success);
        Ok(())
    }

    // TODO: move to state machine
    // /// The address the router should listen on
    // pub fn get_router_address(&self) -> SocketAddr {
    //     self.config_state
    //         .lock()
    //         .expect("could not acquire lock on router config state")
    //         .socket_addr
    //         .unwrap_or(DEFAULT_ROUTER_SOCKET_ADDR)
    // }

    // /// The path the router should listen on
    // pub fn get_router_listen_path(&self) -> String {
    //     self.config_state
    //         .lock()
    //         .expect("could not acquire lock on router config state")
    //         .listen_path
    //         .clone()
    // }

    // /// Get the name of the interprocess socket address to communicate with other rover dev sessions
    // pub fn get_ipc_address(&self) -> RoverResult<String> {
    //     let socket_name = format!("supergraph-{}.sock", self.get_router_address());
    //     {
    //         use interprocess::local_socket::NameTypeSupport::{self, *};
    //         let socket_prefix = match NameTypeSupport::query() {
    //             OnlyPaths | Both => "/tmp/",
    //             OnlyNamespaced => "@",
    //         };
    //         Ok(format!("{}{}", socket_prefix, socket_name))
    //     }
    // }

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

    pub fn into_stream(self) -> impl Stream<Item = Event> {
        match read_config_state(
            self.input_config_path.clone(),
            self.ip_override.clone(),
            self.port_override.clone(),
        ) {
            Ok(router_config_state) => {
                if let Some(input_config_path) = self.input_config_path.clone() {
                    stream::once(future::ready(router_config_state))
                        .chain(
                            Fs::watch_file(input_config_path.clone()).filter_map(move |_| {
                                let input_config_path = input_config_path.clone();
                                let ip_override = self.ip_override.clone();
                                let port_override = self.port_override.clone();
                                async move {
                                    let result = read_config_state(
                                        Some(input_config_path),
                                        ip_override,
                                        port_override,
                                    );
                                    if let Err(e) = &result {
                                        eprintln!("{e}");
                                    }
                                    result.ok()
                                }
                            }),
                        )
                        .boxed()
                } else {
                    stream::once(future::ready(router_config_state)).boxed()
                }
            }
            Err(err) => {
                eprintln!("{err}");
                stream::empty().boxed()
            }
        }
        .map(move |config| Event::UpdateRouterConfig { config })
        .chain(stream::iter(vec![Event::RemoveRouterConfig]))
    }
}

fn read_config_state(
    input_config_path: Option<Utf8PathBuf>,
    ip_override: Option<IpAddr>,
    port_override: Option<u16>,
) -> RoverResult<RouterConfigState> {
    let mut yaml = input_config_path
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
    let socket_addr = match (ip_override, port_override, yaml_socket_addr) {
        (Some(ip), Some(port), _) => Some(SocketAddr::new(ip, port)),
        (Some(ip), None, yaml) => {
            let mut socket_addr = yaml.unwrap_or(DEFAULT_ROUTER_SOCKET_ADDR);
            socket_addr.set_ip(ip);
            Some(socket_addr)
        }
        (None, Some(port), yaml) => {
            let mut socket_addr = yaml.unwrap_or(DEFAULT_ROUTER_SOCKET_ADDR);
            socket_addr.set_port(port);
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

    if let Some(path) = &input_config_path {
        if Fs::assert_path_exists(path).is_err() {
            eprintln!(
                "{}{path} does not exist, creating a router config from CLI options.",
                Emoji::Action
            );
            Fs::write_file(path, &yaml_string)?;
        }
    }

    Ok(RouterConfigState {
        socket_addr,
        config: yaml_string,
        listen_path,
    })
}
