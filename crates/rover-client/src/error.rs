use thiserror::Error;

/// An enum represent all type of possible failures.
#[derive(Error, Debug)]
pub enum RoverClientError<'a> {
    /// The provided GraphQL was invalid.
    #[error("encountered a graphQL error, registry responded with: {msg}")]
    GraphQLError {
        /// The error message.
        msg: String,
    },
    /// Tried to build a [HeaderMap] with an invalid header value.
    #[error("header value for {header} was not valid: {msg:?}")]
    HeadersError {
        /// The header value that was invalid.
        header: &'a str,
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
