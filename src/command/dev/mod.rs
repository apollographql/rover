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
use crate::RoverResult;

#[cfg(feature = "composition-js")]
use tempdir::TempDir;

#[cfg(feature = "composition-js")]
use crate::command::dev::router::RouterConfigHandler;

use crate::options::{OptionalSubgraphOpts, PluginOpts};

use camino::Utf8PathBuf;
use clap::Parser;
use lazycell::{AtomicLazyCell, LazyCell};
use serde::Serialize;

use std::{convert::TryFrom, net::SocketAddr};

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

    /// The path to a router configuration file. If the file path is empty, a default configuration will be written to that file. This file is then watched for changes and propagated to the router.
    ///
    /// For information on the format of this file, please see https://www.apollographql.com/docs/router/configuration/overview/#yaml-config-file.
    #[arg(long = "router-config")]
    #[serde(skip_serializing)]
    router_config_path: Option<Utf8PathBuf>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    router_config_handler: LazyCell<RouterConfigHandler>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    tmp_config_path: AtomicLazyCell<Utf8PathBuf>,
}

#[cfg(feature = "composition-js")]
impl SupergraphOpts {
    /// Get the router config handler
    fn get_config_handler(&self) -> RoverResult<RouterConfigHandler> {
        let tmp_config_path = self.temp_dir_path()?.join("router_config.yaml");
        if let Some(handler) = self.router_config_handler.borrow() {
            Ok(handler.clone())
        } else {
            let mut config_handler = RouterConfigHandler::new(
                tmp_config_path,
                self.router_config_path.clone(),
                self.supergraph_port,
                self.supergraph_address.clone(),
            );

            config_handler.start()?;

            self.router_config_handler
                .fill(config_handler)
                .expect("Could not overwrite existing router config handler");

            self.get_config_handler()
        }
    }

    /// Get the name of the socket the router should listen on for incoming GraphQL requests
    fn router_socket_addr(&self) -> RoverResult<SocketAddr> {
        self.get_config_handler()?.get_router_socket_address()
    }

    /// Get the temp directory for this session
    fn temp_dir_path(&self) -> RoverResult<&Utf8PathBuf> {
        if let Some(tmp_dir_path) = self.tmp_config_path.borrow() {
            Ok(tmp_dir_path)
        } else {
            let temp_dir = TempDir::new("supergraph")?;
            let tmp_config_path = Utf8PathBuf::try_from(temp_dir.into_path())?;
            self.tmp_config_path
                .fill(tmp_config_path)
                .expect("could not overwrite existing temp directory");
            self.temp_dir_path()
        }
    }

    /// Get the path to the temp router config
    pub fn tmp_router_config_path(&self) -> RoverResult<Utf8PathBuf> {
        Ok(self.get_config_handler()?.tmp_router_config_path())
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

    pub fn supergraph_schema_path(&self) -> RoverResult<Utf8PathBuf> {
        Ok(self.temp_dir_path()?.join("supergraph.graphql"))
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
