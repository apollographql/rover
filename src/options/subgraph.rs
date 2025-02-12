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
    pub subgraph_name: Option<String>,

    /// The URL that the `rover dev` router should use to communicate with a running subgraph (e.g., http://localhost:4000).
    ///
    /// This must be unique to each `rover dev` process and cannot be the same endpoint used by the graph router, which are specified by the `--supergraph-port` and `--supergraph-address` arguments.
    #[arg(long = "url", short = 'u')]
    #[serde(skip_serializing)]
    pub subgraph_url: Option<url::Url>,

    /// The number of seconds between introspection requests to the running subgraph.
    /// Only used when the `--schema` argument is not passed.
    /// The default value is 1 second.
    #[arg(long = "polling-interval", short = 'i', default_value = "1")]
    #[serde(skip_serializing)]
    pub subgraph_polling_interval: u64,

    /// The number of times to retry a subgraph if an error is detected from it
    /// The default value is 0.
    #[arg(long = "subgraph-retries", short = 'r', default_value = "0")]
    #[serde(skip_serializing)]
    pub subgraph_retries: u64,
}
