#[cfg(feature = "composition-js")]
mod compose;

#[cfg(feature = "composition-js")]
mod context;

#[cfg(feature = "composition-js")]
mod introspect;

#[cfg(feature = "composition-js")]
mod router;

#[cfg(feature = "composition-js")]
mod schema;

#[cfg(feature = "composition-js")]
mod socket;

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

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};

use crate::{
    error::RoverError,
    options::{OptionalSubgraphOpt, PluginOpts},
    Result, Suggestion,
};
use reqwest::Url;
use saucer::{clap, Parser, Utf8PathBuf};
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
    pub schema_opts: SchemaOpts,

    #[clap(flatten)]
    pub subgraph_opt: OptionalSubgraphOpt,

    #[clap(flatten)]
    pub supergraph_opts: SupergraphOpts,
}

#[derive(Debug, Parser, Serialize, Clone, Copy)]
pub struct SupergraphOpts {
    /// The port the graph router should listen on.
    #[clap(long, short = 'p', default_value = "3000")]
    port: u16,

    /// The IP address the graph router should listen on.
    #[clap(long, default_value_t = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    ip: IpAddr,
}

impl SupergraphOpts {
    pub fn listen_addr(&self) -> Result<SocketAddr> {
        let socket_addr = SocketAddr::new(self.ip, self.port);
        if let Err(e) = TcpListener::bind(socket_addr) {
            let e = saucer::Error::new(e).context("{} is already in use");
            let mut err = RoverError::new(e);
            err.set_suggestion(Suggestion::Adhoc(
                "pass an unused port to `--port`".to_string(),
            ));
            Err(err)
        } else {
            Ok(socket_addr)
        }
    }
}

#[derive(Debug, Parser, Serialize)]
pub struct SchemaOpts {
    /// The URL that the `rover dev` router should use to communicate with this running subgraph (e.g., http://localhost:4000).
    ///
    /// This must be unique to each `rover dev` session and cannot be the same endpoint used by the graph router, which are specified by the `--ip` and `--port` arguments.
    #[clap(long = "url", short = 'u')]
    #[serde(skip_serializing)]
    pub subgraph_url: Option<Url>,

    /// The path to a GraphQL schema file that `rover dev` will use as this subgraph's schema.
    ///
    /// If this argument is passed, `rover dev` does not periodically introspect the running subgraph to obtain its schema.
    /// Instead, it watches the file at the provided path and recomposes the supergraph schema whenever changes occur.
    #[clap(long = "schema", short = 's')]
    #[serde(skip_serializing)]
    pub subgraph_schema_path: Option<Utf8PathBuf>,
}
