#[cfg(feature = "composition-js")]
mod compose;

#[cfg(feature = "composition-js")]
mod introspect;

#[cfg(feature = "composition-js")]
mod router;

#[cfg(feature = "composition-js")]
mod schema;

#[cfg(feature = "composition-js")]
mod protocol;

#[cfg(feature = "composition-js")]
mod netstat;

#[cfg(feature = "composition-js")]
mod watcher;

#[cfg(feature = "composition-js")]
mod do_dev;

#[cfg(not(feature = "composition-js"))]
mod no_dev;

#[cfg(feature = "composition-js")]
use std::str::FromStr;

#[cfg(feature = "composition-js")]
use crate::RoverResult;

#[cfg(feature = "composition-js")]
use anyhow::Context;

#[cfg(feature = "composition-js")]
use rover_std::Fs;

#[cfg(feature = "composition-js")]
use tempdir::TempDir;

use crate::options::{OptionalSubgraphOpts, PluginOpts};

use camino::Utf8PathBuf;
use clap::Parser;
use lazycell::AtomicLazyCell;
use serde::Serialize;

use std::net::SocketAddr;

#[derive(Debug, Serialize, Parser)]
pub struct Dev {
    #[clap(flatten)]
    pub(crate) opts: DevOpts,
}

#[derive(Debug, Serialize, Parser)]
pub struct DevOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    #[clap(flatten)]
    pub subgraph_opts: OptionalSubgraphOpts,

    #[clap(flatten)]
    pub supergraph_opts: SupergraphOpts,
}

#[derive(Debug, Parser, Serialize, Clone)]
pub struct SupergraphOpts {
    /// The port the graph router should listen on.
    ///
    /// If you start multiple `rover dev` processes on the same address and port, they will communicate with each other.
    ///
    /// If you start multiple `rover dev` processes with different addresses and ports, they will not communicate with each other.
    #[arg(long, short = 'p')]
    supergraph_port: Option<u16>,

    /// The address the graph router should listen on.
    ///
    /// If you start multiple `rover dev` processes on the same address and port, they will communicate with each other.
    ///
    /// If you start multiple `rover dev` processes with different addresses and ports, they will not communicate with each other.
    #[arg(long)]
    supergraph_address: Option<String>,

    /// The path to a router configuration file. If the file is empty, a default configuration will be written to that file. This file is not watched. To reload changes to this file, you will need to restart `rover dev`.
    ///
    /// For information on the format of this file, please see https://www.apollographql.com/docs/router/configuration/overview/#yaml-config-file.
    #[arg(long = "router-config")]
    #[serde(skip_serializing)]
    router_config_path: Option<Utf8PathBuf>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    resolved_router_address: AtomicLazyCell<SocketAddr>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    tmp_router_config_path: AtomicLazyCell<Utf8PathBuf>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    temp_dir_path: AtomicLazyCell<Utf8PathBuf>,
}

#[cfg(feature = "composition-js")]
impl SupergraphOpts {
    /// Get the name of the socket the router should listen on for incoming GraphQL requests
    fn router_socket_addr(&self) -> RoverResult<SocketAddr> {
        if let Some(socket_addr) = self.resolved_router_address.borrow() {
            Ok(*socket_addr)
        } else {
            let maybe_supergraph_listen_config =
                if let Some(router_config) = &self.router_config_path {
                    let contents = Fs::read_file(router_config)?;
                    let yaml: serde_yaml::Mapping = serde_yaml::from_str(&contents)
                        .with_context(|| format!("'{router_config}' is not valid YAML"))?;
                    yaml.get("supergraph")
                        .and_then(|s| s.as_mapping())
                        .and_then(|s| s.get("listen"))
                        .and_then(|l| l.as_str())
                        .and_then(|socket_addr| {
                            let socket_addr: Vec<&str> = socket_addr.split(":").collect();
                            if socket_addr.len() != 2 {
                                None
                            } else {
                                Some((socket_addr[0].to_string(), socket_addr[1].to_string()))
                            }
                        })
                } else {
                    None
                };

            // resolve address and port from the options
            // precedence is:
            // 1) `--supergraph-address` and `--supergraph-port`
            // 2) `--router-config`
            // 3) default of 127.0.0.1:3000
            let address = self.supergraph_address.clone().unwrap_or_else(|| {
                if let Some((address, _)) = &maybe_supergraph_listen_config {
                    address.to_string()
                } else {
                    "127.0.0.1".to_string()
                }
            });

            let port = self
                .supergraph_port
                .map(|p| p.to_string())
                .unwrap_or_else(|| {
                    if let Some((_, port)) = &maybe_supergraph_listen_config {
                        port.to_string()
                    } else {
                        "3000".to_string()
                    }
                });

            let socket_candidate = format!("{address}:{port}");
            let socket_addr = SocketAddr::from_str(&socket_candidate)
                .with_context(|| format!("{} is not a valid socket address", &socket_candidate))?;

            self.resolved_router_address
                .fill(socket_addr)
                .expect("Could not overwrite existing router address");
            self.router_socket_addr()
        }
    }

    /// Get the name of the interprocess socket address to communicate with other rover dev sessions
    pub fn ipc_socket_addr(&self) -> RoverResult<String> {
        let socket_name = format!("supergraph-{}.sock", self.router_socket_addr()?);
        {
            use interprocess::local_socket::NameTypeSupport::{self, *};
            let socket_prefix = match NameTypeSupport::query() {
                OnlyPaths | Both => "/tmp/",
                OnlyNamespaced => "@",
            };
            Ok(format!("{}{}", socket_prefix, socket_name))
        }
    }

    /// Get the path to the tmp router config,
    /// creating one from user specified config and applying CLI options
    pub fn tmp_router_config_path(&self) -> RoverResult<Utf8PathBuf> {
        if let Some(config_path) = self.tmp_router_config_path.borrow() {
            Ok(config_path.clone())
        } else {
            let contents = format!(
                r#"supergraph:
    listen: '{0}'
"#,
                &self.router_socket_addr()?
            );

            let config_path = self.temp_dir_path()?.join("router_config.yaml");
            Fs::write_file(&config_path, contents).context("could not create router config")?;

            self.tmp_router_config_path
                .fill(config_path)
                .expect("Could not overwrite existing router config");
            self.tmp_router_config_path()
        }
    }

    pub fn supergraph_schema_path(&self) -> RoverResult<Utf8PathBuf> {
        Ok(self.temp_dir_path()?.join("supergraph.graphql"))
    }

    fn temp_dir_path(&self) -> RoverResult<Utf8PathBuf> {
        if let Some(temp_dir_path) = self.temp_dir_path.borrow() {
            Ok(temp_dir_path.clone())
        } else {
            // create a temp directory for the composed supergraph
            let temp_dir = TempDir::new("supergraph")?;
            let temp_dir_path = Utf8PathBuf::try_from(temp_dir.into_path())?;

            self.temp_dir_path
                .fill(temp_dir_path)
                .expect("Could not overwrite existing temp directory path");
            self.temp_dir_path()
        }
    }
}

lazy_static::lazy_static! {
    pub(crate) static ref OVERRIDE_DEV_ROUTER_VERSION: Option<String> =
      std::env::var("APOLLO_ROVER_DEV_ROUTER_VERSION").ok();

    // this number should be mapped to the federation version used by the router
    // https://www.apollographql.com/docs/router/federation-version-support/#support-table
    pub(crate) static ref OVERRIDE_DEV_COMPOSITION_VERSION: Option<String> =
        std::env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION").ok();
}
