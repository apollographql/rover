use camino::Utf8PathBuf;
use clap::{self, Parser};
use serde::Deserialize;
use url::Url;

#[derive(Debug, Clone, Deserialize, Parser)]
pub struct SubgraphOpt {
    /// The name of the subgraph.
    #[arg(long = "name")]
    pub subgraph_name: String,
}

#[derive(Debug, Clone, Deserialize, Parser)]
pub struct OptionalSubgraphOpts {
    /// The name of the subgraph.
    ///
    /// This must be unique to each `rover dev` process.
    #[arg(long = "name", short = 'n')]
    pub subgraph_name: Option<String>,

    /// The URL that the `rover dev` router should use to communicate with a running subgraph (e.g., http://localhost:4000).
    ///
    /// This must be unique to each `rover dev` process and cannot be the same endpoint used by the graph router, which are specified by the `--supergraph-port` and `--supergraph-address` arguments.
    #[arg(long = "url", short = 'u')]
    pub subgraph_url: Option<Url>,

    /// The path to a GraphQL schema file that `rover dev` will use as this subgraph's schema.
    ///
    /// If this argument is passed, `rover dev` does not periodically introspect the running subgraph to obtain its schema.
    /// Instead, it watches the file at the provided path and recomposes the supergraph schema whenever changes occur.
    #[arg(long = "schema", short = 's', value_name = "SCHEMA_PATH")]
    pub subgraph_schema_path: Option<Utf8PathBuf>,

    /// The number of seconds between introspection requests to the running subgraph.
    /// Only used when the `--schema` argument is not passed.
    /// The default value is 1 second.
    #[arg(
        long = "polling-interval",
        short = 'i',
        default_value = "1",
        conflicts_with = "subgraph_schema_path"
    )]
    pub subgraph_polling_interval: u64,

    /// The number of times to retry a subgraph if an error is detected from it
    /// The default value is 0.
    #[arg(long = "subgraph-retries", short = 'r', default_value = "0")]
    pub subgraph_retries: u64,
}
