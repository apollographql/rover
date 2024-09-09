use bytes::Bytes;
use http::StatusCode;

#[derive(thiserror::Error, Debug)]
pub enum HttpServiceError {
    #[error("Bad Status code: {status_code}")]
    BadStatusCode {
        status_code: StatusCode,
        data: Bytes,
    },
    #[error("HTTP Error: {:?}", .0)]
    Http(#[from] http::Error),
    #[error("Request was cancelled: {:?}", .0)]
    Cancelled(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Sender channel was closed: {:?}", .0)]
    Closed(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Incomplete message: {:?}", .0)]
    Incomplete(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Request timed out: {:?}", .0)]
    TimedOut(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Decode error: {:?}", .0)]
    Decode(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Body error: {:?}", .0)]
    Body(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Connect error: {:?}", .0)]
    Connect(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Unexpected HTTP error: {:?}", .0)]
    NoCACerts(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Unexpected HTTP error: {:?}", .0)]
    Unexpected(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl HttpServiceError {
    pub fn is_connect(&self) -> bool {
        matches!(self, HttpServiceError::Connect(_))
    }
    pub fn is_timeout(&self) -> bool {
        matches!(self, HttpServiceError::TimedOut(_))
    }
    pub fn is_decode(&self) -> bool {
        matches!(self, HttpServiceError::Decode(_))
    }
    pub fn is_status(&self) -> bool {
        matches!(self, HttpServiceError::BadStatusCode { .. })
    }
}
