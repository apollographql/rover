use reqwest::Url;
use thiserror::Error;

/// RoverClientError represents all possible failures that can occur during a client request.
#[derive(Error, Debug)]
pub enum RoverClientError {
    /// The provided GraphQL was invalid.
    #[error("{msg}")]
    GraphQl {
        /// The encountered GraphQL error.
        msg: String,
    },

    /// Failed to parse Introspection Response coming from server.
    #[error("{msg}")]
    IntrospectionError {
        /// Introspection Error coming from schema encoder.
        msg: String,
    },

    /// Tried to build a [HeaderMap] with an invalid header name.
    #[error("Invalid header name")]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),

    /// Tried to build a [HeaderMap] with an invalid header value.
    #[error("Invalid header value")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    /// Invalid JSON in response body.
    #[error("Could not parse JSON")]
    InvalidJson(#[from] serde_json::Error),

    /// Encountered an error handling the received response.
    #[error("{msg}")]
    AdhocError {
        /// The error message.
        msg: String,
    },

    /// Encountered a 400-599 error from an endpoint.
    #[error("Unable to get a response from an endpoint. Client returned an error.\n\n{msg}")]
    ClientError {
        /// Error message from client.
        msg: String,
    },

    /// The user provided an invalid subgraph name.
    #[error("Could not find subgraph \"{invalid_subgraph}\".")]
    NoSubgraphInGraph {
        /// The invalid subgraph name
        invalid_subgraph: String,

        /// A list of valid subgraph names
        // this is not used in the error message, but can be accessed
        // by application-level error handlers
        valid_subgraphs: Vec<String>,
    },

    /// The Studio API could not find a variant for a graph
    #[error(
        "The graph registry does not contain variant \"{invalid_variant}\" for graph \"{graph}\""
    )]
    NoSchemaForVariant {
        /// The name of the graph.
        graph: String,

        /// The non-existent variant.
        invalid_variant: String,

        /// Valid variants.
        valid_variants: Vec<String>,

        /// Front end URL root.
        frontend_url_root: String,
    },

    #[error("Could not connect to {}.",
        if let Some(url) = .url {
            url.to_string()
        } else {
            "unknown URL".to_string()
        }
    )]
    CouldNotConnect {
        source: reqwest::Error,
        url: Option<Url>,
    },

    /// Encountered an error sending the request.
    #[error(transparent)]
    SendRequest(#[from] reqwest::Error),

    /// when someone provides a bad graph/variant combination or isn't
    /// validated properly, we don't know which reason is at fault for data.service
    /// being empty, so this error tells them to check both.
    #[error("Could not find graph with name \"{graph}\"")]
    NoService { graph: String },

    /// if someone attempts to get a core schema from a supergraph that has
    /// no composition results we return this error.
    #[error("No supergraph SDL exists for \"{graph}\" because its subgraphs failed to compose.")]
    NoCompositionPublishes {
        graph: String,
        composition_errors: Vec<String>,
    },

    /// This error occurs when the Studio API returns no implementing services for a graph
    /// This response shouldn't be possible!
    #[error("The response from Apollo Studio was malformed. Response body contains `null` value for \"{null_field}\"")]
    MalformedResponse { null_field: String },

    #[error("The graph `{graph}` is a non-federated graph. This operation is only possible for federated graphs")]
    ExpectedFederatedGraph { graph: String },

    /// The API returned an invalid ChangeSeverity value
    #[error("Invalid ChangeSeverity.")]
    InvalidSeverity,

    /// This error occurs when a user has a malformed API key
    #[error(
        "The API key you provided is malformed. An API key must have three parts separated by a colon."
    )]
    MalformedKey,

    /// The registry could not find this key
    #[error("The registry did not recognize the provided API key")]
    InvalidKey,

    /// could not parse the latest version
    #[error("Could not get the latest release version")]
    UnparseableReleaseVersion,

    #[error("This endpoint doesn't support subgraph introspection via the Query._service field")]
    SubgraphIntrospectionNotAvailable,
}
