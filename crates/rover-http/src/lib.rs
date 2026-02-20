#![warn(missing_docs)]

//! Provides [`tower`] implementations for HTTP Requests

use std::{fmt::Debug, str::Utf8Error, time::Duration};

/// Install ring as the default rustls crypto provider. This runs automatically
/// as a global constructor in every binary that links rover-http (directly or
/// transitively).
#[ctor::ctor]
fn install_ring_crypto_provider() {
    // .ok() because the provider may already be installed, and that's the only
    // case that causes this to error
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
}

use buildstructor::Builder;
use bytes::Bytes;
use derive_getters::Getters;
pub use http_body::Body;
pub use http_body_util::{BodyExt, Empty, Full};
use tower::{timeout::error::Elapsed, util::BoxCloneService};

pub mod body;
mod error;
pub mod error_on_status;
pub mod extend_headers;
mod reqwest;
pub mod retry;
#[cfg(any(test, feature = "test"))]
pub mod test;
pub mod timeout;

pub use error::HttpServiceError;
pub use reqwest::{ReqwestService, ReqwestServiceFactory};

/// Ease-of-use synonym for the request type this crate operates on
pub type HttpRequest = http::Request<Full<Bytes>>;
/// Ease-of-use synonym for the response type this crate operates on
pub type HttpResponse<T = Full<Bytes>> = http::Response<T>;
/// Ease-of-use synonym for the [`Service`] type this crate provides
pub type HttpService = BoxCloneService<HttpRequest, HttpResponse, HttpServiceError>;

/// Object that creates an HttpService on-demand
/// This is useful because [`Service`]s must be `mut` to be useful, and this requirement
/// works its way up the chain in a way that can be problematic
/// This produces a [`Service`] as a raw artifact, rather than encapsulating it as something
/// else, such as some type of Client, in order to allow for [`tower`] layering upon production
pub trait HttpServiceFactory {
    /// Produces an [`HttpService`]
    #[allow(clippy::result_large_err)]
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
            Ok(_) => HttpServiceError::TimedOut,
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
