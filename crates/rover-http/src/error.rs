use bytes::Bytes;
use http::StatusCode;
use http_body_util::Full;

/// Errors occuring from the use of an [`HttpService`]
#[derive(thiserror::Error, Debug)]
pub enum HttpServiceError {
    /// Error that occurs as a result of a bad [`StatusCode`] returning
    /// This does not appear by default, it only appears when used with [`ErrorOnStatus`]
    #[error("Bad Status code: {status_code}")]
    BadStatusCode {
        /// The [`StatusCode`] returned by the response
        status_code: StatusCode,
        /// The [`Response`] that generated the [`StatusCode`], for potentially further handling
        response: http::Response<Full<Bytes>>,
    },
    /// Errors that may occur from the [`http`] crate. This is generally relegated to
    /// parsing of things like [`Uri`]s or header names/values
    #[error("HTTP Error: {:?}", .0)]
    Http(#[from] http::Error),
    /// The request was cancelled
    #[error("Request was cancelled.")]
    Cancelled(Box<dyn std::error::Error + Send + Sync + 'static>),
    /// The connection closed unexpectedly
    #[error("Sender channel was closed.")]
    Closed(Box<dyn std::error::Error + Send + Sync + 'static>),
    /// Request timed out
    #[error("Request timed out")]
    TimedOut,
    /// Error decoding the request/response body
    #[error("Decode error")]
    Decode(Box<dyn std::error::Error + Send + Sync + 'static>),
    /// A general error with the Body
    #[error("Body error")]
    Body(Box<dyn std::error::Error + Send + Sync + 'static>),
    /// Error connecting
    #[error("Connect error")]
    Connect(Box<dyn std::error::Error + Send + Sync + 'static>),
    /// An unexpected error
    #[error("Unexpected HTTP error. {}", .0)]
    Unexpected(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl HttpServiceError {
    /// The error is caused by a bad connection
    pub const fn is_connect(&self) -> bool {
        matches!(self, HttpServiceError::Connect(_))
    }
    /// The error is caused by a timeout
    pub const fn is_timeout(&self) -> bool {
        matches!(self, HttpServiceError::TimedOut)
    }
    /// The error is related to decoding the response
    pub const fn is_decode(&self) -> bool {
        matches!(
            self,
            HttpServiceError::Decode(_) | HttpServiceError::Body(_)
        )
    }
    /// The error is because of a bad [`StatusCode`]
    pub const fn is_status(&self) -> bool {
        matches!(self, HttpServiceError::BadStatusCode { .. })
    }
}
