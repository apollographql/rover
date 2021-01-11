use thiserror::Error;

/// RoverClientError represents all possible failures that can occur during a client request.
#[derive(Error, Debug)]
pub enum RoverClientError {
    /// The provided GraphQL was invalid.
    #[error("encountered a GraphQL error, registry responded with: {msg}")]
    GraphQL { msg: String },

    /// Tried to build a [HeaderMap] with an invalid header name.
    #[error("invalid header name")]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),

    /// Tried to build a [HeaderMap] with an invalid header value.
    #[error("invalid header value")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    /// Invalid JSON in response body.
    #[error("could not parse JSON")]
    InvalidJSON(#[from] serde_json::Error),

    /// Encountered an error handling the received response.
    #[error("encountered an error handling the response: {msg}")]
    HandleResponse {
        /// The error message.
        msg: String,
    },

    /// Encountered an error sending the request.
    #[error("encountered an error while sending a request")]
    SendRequest(#[from] reqwest::Error),

    /// This error occurs when there are no `body.errors` but `body.data` is
    /// also empty. In proper GraphQL responses, there should _always_ be either
    /// body.errors or body.data
    #[error("The response from the server was malformed. There was no data found in the reponse body. This is likely an error in GraphQL execution")]
    NoData,

    /// when someone provides a bad service/variant combination or isn't
    /// validated properly, we don't know which reason is at fault for data.service
    /// being empty, so this error tells them to check both.
    #[error("No graph found. Either the graph@variant combination wasn't found or your API key is invalid.")]
    NoService,

    /// This error occurs when the Studio API returns no composition errors AND
    /// no check result. This response shouldn't be possible!
    #[error(
        "The response from the server was malformed, there was no data from the check operation."
    )]
    NoCheckData,

    #[error("The graph `{graph_name}` is a non-federated graph. This operation is only possible for federated graphs")]
    ExpectedFederatedGraph { graph_name: String },

    /// The API returned an invalid ChangeSeverity value
    #[error("Invalid ChangeSeverity.")]
    InvalidSeverity,
}
