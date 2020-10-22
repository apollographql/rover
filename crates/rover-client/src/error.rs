use thiserror::Error;

/// RoverClientError represents all possible failures that can occur during a client request.
#[derive(Error, Debug)]
pub enum RoverClientError {
    /// The provided GraphQL was invalid.
    #[error("encountered a GraphQL error, registry responded with: {msg}")]
    GraphQLError {
        /// The error message.
        msg: String,
    },
    /// Tried to build a [HeaderMap] with an invalid header value.
    #[error("header value was not valid: {msg:?}")]
    HeadersError {
        /// The error message.
        msg: http::header::InvalidHeaderValue,
    },
    /// Encountered an error handling the received response.
    #[error("encountered an error handling the response: {msg}")]
    ResponseError {
        /// The error message.
        msg: String,
    },
    /// Encountered an error sending the request.
    #[error("encountered an error sending the request: {msg:?}")]
    RequestError {
        /// The error message from [reqwest].
        msg: reqwest::Error,
    },
}
