#![warn(missing_docs)]

//! Provides [`tower`] implementations for HTTP Requests

use std::{fmt::Debug, str::Utf8Error, time::Duration};

use buildstructor::Builder;
use bytes::Bytes;
use derive_getters::Getters;
use http_body_util::Full;

use tower::{timeout::error::Elapsed, util::BoxCloneService, Layer};

pub mod body;
mod error;
pub mod error_on_status;
pub mod extend_headers;
mod reqwest;
pub mod retry;

pub use error::HttpServiceError;
pub use reqwest::{ReqwestService, ReqwestServiceFactory};

/// Ease-of-use synonym for the request type this crate operates on
pub type HttpRequest = http::Request<Full<Bytes>>;
/// Ease-of-use synonym for the response type this crate operates on
pub type HttpResponse = http::Response<Bytes>;
/// Ease-of-use synonym for the [`Service`] type this crate provides
pub type HttpService = BoxCloneService<HttpRequest, HttpResponse, HttpServiceError>;

/// Object that creates an HttpService on-demand
/// This is useful because [`Service`]s must be `mut` to be useful, and this requirement
/// works its way up the chain in a way that can be problematic
/// This produces a [`Service`] as a raw artifact, rather than encapsulating it as something
/// else, such as some type of Client, in order to allow for [`tower`] layering upon production
pub trait HttpServiceFactory {
    /// Produces an [`HttpService`]
    fn create(&self) -> Result<HttpService, HttpServiceError>;
}

/// Configuration object for constructing an [`HttpService`].
/// This is intended to be agnostic to the underlying implementation
#[derive(Clone, Debug, Builder, Default, Getters)]
pub struct HttpServiceConfig {
    accept_invalid_certificates: Option<bool>,
    accept_invalid_hostnames: Option<bool>,
    timeout: Option<Duration>,
}

impl From<Box<dyn std::error::Error + Send + Sync>> for HttpServiceError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        match err.downcast::<Elapsed>() {
            Ok(err) => HttpServiceError::TimedOut(err),
            Err(err) => match err.downcast::<HttpServiceError>() {
                Ok(err) => *err,
                Err(err) => HttpServiceError::Unexpected(err),
            },
        }
    }
}

impl From<Utf8Error> for HttpServiceError {
    fn from(value: Utf8Error) -> Self {
        HttpServiceError::Decode(Box::new(value))
    }
}
