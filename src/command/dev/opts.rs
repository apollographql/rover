use crate::options::PluginOpts;
use reqwest::Url;
use saucer::{clap, Parser, Utf8PathBuf};
use serde::Serialize;

#[derive(Debug, Parser, Serialize)]
pub struct SchemaOpts {
    /// Url of a running subgraph that a graph router can send operations to
    /// (often a localhost endpoint).
    #[clap(long)]
    #[serde(skip_serializing)]
    pub url: Option<Url>,

    /// Command to run a subgraph that a graph router can send operations to
    #[clap(long)]
    #[serde(skip_serializing)]
    pub command: Option<String>,

    /// Path to an SDL file for a running subgraph that a graph router can send operations to
    #[clap(long, short = 's')]
    #[serde(skip_serializing)]
    pub schema: Option<Utf8PathBuf>,
    // TODO: this is semi-blocked because the router doesn't provide this as a CLI flag
    // /// The port to run the local supergraph on. Each port gets its own namespace,
    // /// meaning if you run multiple `rover dev` instances with the same `--port`,
    // /// they will attach themselves to each other. If they have different ports,
    // /// they will create a completely new supergraph.
    // #[clap(long, short = 'p', default_value_t = 4000)]
    // pub supergraph_port: usize,
}

#[derive(Debug, Serialize, Parser)]
pub struct DevOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    #[clap(flatten)]
    pub schema_opts: SchemaOpts,

    #[clap(long)]
    pub name: Option<String>,
}
