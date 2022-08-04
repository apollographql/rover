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

    /// Encountered a 400-599 error from an endpoint.
    #[error("Unable to get a response from an endpoint. Client returned an error.\n\n{msg}")]
    ClientError {
        /// Error message from client.
        msg: String,
    },

    /// Encountered an error sending the request.
    #[error(transparent)]
    SendRequest(#[from] reqwest::Error),

    /// This error occurs when the GraphQL API returns null for a non-nullable field
    #[error(
        "The response  was malformed. Response body contains `null` value for \"{null_field}\""
    )]
    MalformedResponse { null_field: String },

    /// This error occurs when a user has a malformed API key
    #[error(
        "The API key you provided is malformed. An API key must have three parts separated by a colon."
    )]
    MalformedKey,
}
