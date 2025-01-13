use camino::Utf8PathBuf;
use clap::{self, Parser};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct SubgraphOpt {
    /// The name of the subgraph.
    #[arg(long = "name")]
    #[serde(skip_serializing)]
    pub subgraph_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct OptionalSubgraphOpts {
    /// The name of the subgraph.
    ///
    /// This must be unique to each `rover dev` process.
    #[arg(long = "name", short = 'n')]
    #[serde(skip_serializing)]
    subgraph_name: Option<String>,

    /// The URL that the `rover dev` router should use to communicate with a running subgraph (e.g., http://localhost:4000).
    ///
    /// This must be unique to each `rover dev` process and cannot be the same endpoint used by the graph router, which are specified by the `--supergraph-port` and `--supergraph-address` arguments.
    #[arg(long = "url", short = 'u')]
    #[serde(skip_serializing)]
    subgraph_url: Option<String>,

    /// The path to a GraphQL schema file that `rover dev` will use as this subgraph's schema.
    ///
    /// If this argument is passed, `rover dev` does not periodically introspect the running subgraph to obtain its schema.
    /// Instead, it watches the file at the provided path and recomposes the supergraph schema whenever changes occur.
    #[arg(long = "schema", short = 's', value_name = "SCHEMA_PATH")]
    #[serde(skip_serializing)]
    subgraph_schema_path: Option<Utf8PathBuf>,

    /// The number of seconds between introspection requests to the running subgraph.
    /// Only used when the `--schema` argument is not passed.
    /// The default value is 1 second.
    #[arg(
        long = "polling-interval",
        short = 'i',
        default_value = "1",
        conflicts_with = "subgraph_schema_path"
    )]
    #[serde(skip_serializing)]
    pub subgraph_polling_interval: u64,

    /// The number of times to retry a subgraph if an error is detected from it
    /// The default value is 0.
    #[arg(long = "subgraph-retries", short = 'r', default_value = "0")]
    #[serde(skip_serializing)]
    pub subgraph_retries: u64,
}
