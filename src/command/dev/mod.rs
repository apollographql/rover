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

use std::{net::SocketAddr, str::FromStr};

use crate::{
    options::{OptionalSubgraphOpts, PluginOpts},
    Result,
};
use saucer::{clap, Context, Parser};
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
    /// If you start multiple `rover dev` sessions on the same address and port, they will communicate with each other.
    ///
    /// If you start multiple `rover dev` sessions with different addresses and ports, they will not communicate with each other.
    #[clap(long, short = 'p', default_value = "3000")]
    supergraph_port: u16,

    /// The address the graph router should listen on.
    ///
    /// If you start multiple `rover dev` sessions on the same address and port, they will communicate with each other.
    ///
    #[clap(long, default_value = "127.0.0.1")]
    supergraph_address: String,
}

impl SupergraphOpts {
    pub fn router_socket_addr(&self) -> Result<SocketAddr> {
        let socket_candidate = format!("{}:{}", &self.supergraph_address, &self.supergraph_port);
        Ok(SocketAddr::from_str(&socket_candidate)
            .with_context(|| format!("{} is not a valid socket address", &socket_candidate))?)
    }

    pub fn ipc_socket_addr(&self) -> String {
        let socket_name = format!("supergraph-{}.sock", &self.supergraph_port);
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

// TODO: make this configurable once the router is stable enough
// and there is a way to determine the correct composition version
// to use with a router version
pub(crate) const DEV_ROUTER_VERSION: &str = "1.0.0-rc.1";

// this number should be mapped to the federation version used by the router
// https://www.apollographql.com/docs/router/federation-version-support/#support-table
pub(crate) const DEV_COMPOSITION_VERSION: &str = "2.1.2-alpha.2";
