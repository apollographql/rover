#[cfg(feature = "composition-js")]
mod command;

#[cfg(feature = "composition-js")]
mod compose;

#[cfg(feature = "composition-js")]
mod introspect;

#[cfg(feature = "composition-js")]
mod router;

#[cfg(feature = "composition-js")]
mod schema;

#[cfg(feature = "composition-js")]
mod netstat;

#[cfg(feature = "composition-js")]
mod socket;

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
    /// Url of a running subgraph that a graph router can send operations to, (i.e., http://localhost:4001).
    ///
    /// If you pass a `--command` argument and no `--url` argument,
    /// `rover dev` will attempt to detect the endpoint by doing a scan of your ports. If you find
    /// this takes too long or is unable to detect your GraphQL server, pass the `--url` argument
    /// to skip the auto-detection step.
    #[clap(long = "url", short = 'u')]
    #[serde(skip_serializing)]
    pub subgraph_url: Option<Url>,

    /// Command to run a subgraph that a graph router can send operations to, (i.e., 'npm run start', 'cargo run', 'go run server.go').
    ///
    /// This argument is provided as a convenience to you, you do not have to pass this argument to run `rover dev`,
    /// you can instead start your server independently in another terminal before running `rover dev`.
    #[clap(long = "command")]
    #[serde(skip_serializing)]
    pub subgraph_command: Option<String>,

    /// Path to an SDL file for a running subgraph that a graph router can send operations to.
    ///
    /// If this argument is passed, `rover dev` will not introspect your endpoint every second to retrieve your schema.
    /// Instead, it will instead watch your file system for changes to the schema, only recomposing when necessary.
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
