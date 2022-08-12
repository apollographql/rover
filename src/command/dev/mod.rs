#[cfg(feature = "composition-js")]
mod compose;

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

use crate::options::{OptionalSubgraphOpt, PluginOpts};
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
}

#[derive(Debug, Parser, Serialize)]
pub struct SchemaOpts {
    /// The URL that the `rover dev` router should use to communicate with this running subgraph (e.g., http://localhost:4001).
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
    // TODO: this is semi-blocked because the router doesn't provide this as a CLI flag
    // /// The port to run the local supergraph on. Each port gets its own namespace,
    // /// meaning if you run multiple `rover dev` instances with the same `--port`,
    // /// they will attach themselves to each other. If they have different ports,
    // /// they will create a completely new supergraph.
    // #[clap(long, short = 'p', default_value_t = 4000)]
    // pub supergraph_port: usize,
}
