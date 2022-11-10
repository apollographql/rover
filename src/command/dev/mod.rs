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
mod command;

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
use crate::RoverResult;

#[cfg(feature = "composition-js")]
use anyhow::Context;

use crate::options::{OptionalSubgraphOpts, PluginOpts};
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
    #[arg(long, short = 'p', default_value = "3000")]
    supergraph_port: u16,

    /// The address the graph router should listen on.
    ///
    /// If you start multiple `rover dev` processes on the same address and port, they will communicate with each other.
    ///
    /// If you start multiple `rover dev` processes with different addresses and ports, they will not communicate with each other.
    #[arg(long, default_value = "127.0.0.1")]
    supergraph_address: String,
}

#[cfg(feature = "composition-js")]
impl SupergraphOpts {
    pub fn router_socket_addr(&self) -> RoverResult<SocketAddr> {
        let socket_candidate = format!("{}:{}", &self.supergraph_address, &self.supergraph_port);
        Ok(SocketAddr::from_str(&socket_candidate)
            .with_context(|| format!("{} is not a valid socket address", &socket_candidate))?)
    }

    pub fn ipc_socket_addr(&self) -> String {
        let socket_name = format!(
            "supergraph-{}-{}.sock",
            &self.supergraph_address, &self.supergraph_port
        );
        {
            use interprocess::local_socket::NameTypeSupport::{self, *};
            let socket_prefix = match NameTypeSupport::query() {
                OnlyPaths | Both => "/tmp/",
                OnlyNamespaced => "@",
            };
            format!("{}{}", socket_prefix, socket_name)
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
