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
use std::{net::SocketAddr, str::FromStr};

#[cfg(feature = "composition-js")]
use crate::{RoverError, RoverResult};

#[cfg(feature = "composition-js")]
use anyhow::Context;
use lazycell::AtomicLazyCell;
use rover_std::Fs;
use rover_std::Style;
use tempdir::TempDir;

use crate::{
    options::{OptionalSubgraphOpts, PluginOpts},
    RoverErrorSuggestion,
};

use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

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

    /// The path to a router configuration file. If the file is empty, a default configuration will be written to that file.
    ///
    /// For information on the format of this file, please see https://www.apollographql.com/docs/router/configuration/overview/#yaml-config-file.
    #[arg(long = "router-config", conflicts_with_all = ["supergraph_port", "supergraph_address"])]
    #[serde(skip_serializing)]
    router_config_path: Option<Utf8PathBuf>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    resolved_router_address: AtomicLazyCell<SocketAddr>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    resolved_router_config: AtomicLazyCell<Utf8PathBuf>,

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
            let socket_candidate = if let Some(router_config) = &self.router_config_path {
                let contents = Fs::read_file(router_config)?;
                let yaml: serde_yaml::Mapping = serde_yaml::from_str(&contents)
                    .with_context(|| format!("'{router_config}' is not valid YAML"))?;
                if let Some(socket_addr) = yaml
                    .get("supergraph")
                    .and_then(|s| s.as_mapping())
                    .and_then(|s| s.get("listen"))
                    .and_then(|l| l.as_str())
                {
                    socket_addr.to_string()
                } else {
                    "127.0.0.1:3000".to_string()
                }
            } else {
                format!(
                    "{}:{}",
                    &self
                        .supergraph_address
                        .clone()
                        .unwrap_or_else(|| "127.0.0.1".to_string()),
                    &self.supergraph_port.unwrap_or(3000)
                )
            };

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

    /// Get the path to the router config, creating one if the user did not specify one of their own with --router-config
    pub fn router_config_path(&self) -> RoverResult<Utf8PathBuf> {
        if let Some(config_path) = self.resolved_router_config.borrow() {
            Ok(config_path.clone())
        } else {
            let config_path = if let Some(config_path) = &self.router_config_path {
                Fs::assert_path_exists(config_path).map_err(|e| {
                  let mut err = RoverError::from(e);
                  err.set_suggestion(RoverErrorSuggestion::Adhoc(format!("{} must be a path to a YAML configuration file for the Apollo Router. More information on this configuration file can be found here: {}", Style::Command.paint("--router-config"), Style::Link.paint("https://www.apollographql.com/docs/router/configuration/overview/#yaml-config-file"))));
                  err
                })?;
                config_path.clone()
            } else {
                let config_path = self.temp_dir_path()?.join("router_config.yaml");

                let contents = format!(
                    r#"supergraph:
    listen: '{0}'
"#,
                    &self.router_socket_addr()?
                );
                Fs::write_file(&config_path, contents).context("could not create router config")?;
                config_path
            };

            self.resolved_router_config
                .fill(config_path)
                .expect("Could not overwrite existing router config");
            self.router_config_path()
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
    pub(crate) static ref DEV_ROUTER_VERSION: String =
      std::env::var("APOLLO_ROVER_DEV_ROUTER_VERSION").unwrap_or_else(|_| "1.3.0".to_string());

    // this number should be mapped to the federation version used by the router
    // https://www.apollographql.com/docs/router/federation-version-support/#support-table
    pub(crate) static ref DEV_COMPOSITION_VERSION: String =
        std::env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION").unwrap_or_else(|_| "2.1.4".to_string());
}
