#![warn(missing_docs)]

//! Provides [`tower`] implementations for HTTP Requests

use std::{fmt::Debug, str::Utf8Error, sync::Arc, time::Duration};

use buildstructor::Builder;
use bytes::Bytes;
use derive_getters::Getters;
use http_body_util::Full;

use tokio::sync::Mutex;
use tower::{
    make::Shared, timeout::error::Elapsed, util::BoxCloneService, Layer, MakeService, Service,
    ServiceBuilder, ServiceExt,
};

pub mod body;
mod error;
pub mod error_on_status;
pub mod extend_headers;
mod reqwest;
pub mod retry;

pub use error::HttpServiceError;
pub use reqwest::ReqwestService;

/// Ease-of-use synonym for the request type this crate operates on
pub type HttpRequest = http::Request<Full<Bytes>>;
/// Ease-of-use synonym for the response type this crate operates on
pub type HttpResponse = http::Response<Bytes>;
/// Ease-of-use synonym for the [`Service`] type this crate provides
pub type HttpService = BoxCloneService<HttpRequest, HttpResponse, HttpServiceError>;

/// Constructs [`HttpService`]s as a [`Service`]
#[derive(Clone, Debug)]
pub struct HttpServiceFactory {
    factory: Arc<Mutex<Shared<HttpService>>>,
}

impl HttpServiceFactory {
    /// Provides a new [`HttpService`]
    pub async fn get(&self) -> HttpService {
        let mut factory = self.factory.lock().await;
        factory.make_service(()).await.expect("Expected Infallible")
    }

    /// Provides an [`HttpServiceFactory`] that produces [`HttpService`]s with the provided [`Layer`]
    pub async fn with_layer<L, S, E>(&self, layer: L) -> HttpServiceFactory
    where
        L: Layer<HttpService, Service = S>,
        S: Service<HttpRequest, Response = HttpResponse, Error = E> + Clone + Send + 'static,
        S::Future: Send,
        E: Into<HttpServiceError>,
    {
        let http_service = self.get().await;
        let http_service = ServiceBuilder::new()
            .map_err(|err: E| err.into())
            .layer(layer)
            .service(http_service)
            .boxed_clone();
        HttpServiceFactory::from(http_service)
    }
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

impl From<HttpService> for HttpServiceFactory {
    fn from(value: HttpService) -> Self {
        HttpServiceFactory {
            factory: Arc::new(Mutex::new(Shared::new(value))),
        }
    }
}
