use std::path::PathBuf;

use camino::Utf8PathBuf;

/// Errors that may occur as a result of resolving subgraphs
#[derive(thiserror::Error, Debug)]
pub enum ResolveSubgraphError {
    /// Occurs when the subgraph schema file cannot found relative to the supplied
    /// supergraph config file
    #[error("Could not find schema file ({path}) relative to ({supergraph_config_path}) for subgraph `{subgraph_name}`")]
    FileNotFound {
        /// The subgraph name that failed to be resolved
        subgraph_name: String,
        /// Supplied path to the supergraph config file
        supergraph_config_path: Utf8PathBuf,
        /// Supplied path to the subgraph schema file
        path: PathBuf,
        /// The source error
        source: std::io::Error,
    },
    /// Occurs as a result of an IO error
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Occurs as a result of a rover_std::Fs error
    #[error(transparent)]
    Fs(Box<dyn std::error::Error + Send + Sync>),
    /// Occurs when a introspection against a subgraph fails
    #[error("Failed to introspect the subgraph {subgraph_name}.")]
    IntrospectionError {
        /// The subgraph name that failed to be resolved
        subgraph_name: String,
        /// The source error
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Occurs when a supplied graph ref cannot be parsed
    #[error("Invalid graph ref: {graph_ref}")]
    InvalidGraphRef {
        /// The supplied graph ref
        graph_ref: String,
        /// The source error
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Occurs when fetching a remote subgraph fails
    #[error("Failed to fetch the sdl for subgraph `{}` from remote.\n {}", .subgraph_name, .source)]
    FetchRemoteSdlError {
        /// The name of the subgraph that failed to be resolved
        subgraph_name: String,
        /// The source error
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Occurs when a supergraph config filepath waqs expected but not found
    #[error("Failed to find the supergraph config, which is required when resolving schemas in a file relative to a supergraph config")]
    SupergraphConfigMissing,
    /// Invalid input from the user in response to prompting
    #[error("Invalid input: {input}")]
    InvalidCliInput {
        /// The invalid input from the user
        input: String,
    },
}
