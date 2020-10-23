use thiserror::Error;

/// RoverClientError represents all possible failures that can occur during a client request.
#[derive(Error, Debug)]
pub enum RoverClientError {
    /// The provided GraphQL was invalid.
    #[error("encountered a GraphQL error, registry responded with: {msg}")]
    /// The error message.
    GraphQL { msg: String },
    /// Tried to build a [HeaderMap] with an invalid header value.
    #[error("invalid header value")]
    InvalidHeader(#[from] http::header::InvalidHeaderValue),
    /// Encountered an error handling the received response.
    #[error("encountered an error handling the response: {msg}")]
    HandleResponse {
        /// The error message.
        msg: String,
    },
    /// Encountered an error sending the request.
    #[error("encountered an error while sending a request")]
    SendRequest(#[from] reqwest::Error),
}
