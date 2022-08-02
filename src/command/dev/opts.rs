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
    pub subgraph_url: Option<Url>,

    /// Command to run a subgraph that a graph router can send operations to
    #[clap(long)]
    #[serde(skip_serializing)]
    pub subgraph_command: Option<String>,

    /// Path to an SDL file for a running subgraph that a graph router can send operations to
    #[clap(long, short = 's')]
    #[serde(skip_serializing)]
    pub subgraph_schema: Option<Utf8PathBuf>,
}

#[derive(Debug, Serialize, Parser)]
pub struct DevOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    #[clap(flatten)]
    pub schema_opts: SchemaOpts,

    #[clap(long)]
    pub subgraph_name: Option<String>,
}
