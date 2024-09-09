use std::{
    fmt::Debug,
    future::Future,
    pin::Pin,
    str::Utf8Error,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use buildstructor::{buildstructor, Builder};
use bytes::Bytes;
use derive_getters::Getters;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper_rustls::HttpsConnector;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use tokio::sync::Mutex;
use tower::{
    make::Shared, timeout::error::Elapsed, util::BoxCloneService, Layer, MakeService, Service,
    ServiceBuilder, ServiceExt,
};
use tower_http::{
    classify::{NeverClassifyEos, StatusInRangeAsFailures, StatusInRangeFailureClass},
    decompression::{DecompressionBody, DecompressionLayer},
    follow_redirect::FollowRedirectLayer,
    trace::{ResponseBody, TraceLayer},
};

pub mod dangerous;
mod error;
pub mod retry;

pub use error::HttpServiceError;

pub type HttpService =
    BoxCloneService<http::Request<Full<Bytes>>, http::Response<Bytes>, HttpServiceError>;

#[derive(Clone, Debug)]
pub struct HttpServiceFactory {
    factory: Arc<Mutex<Shared<HttpService>>>,
}

impl HttpServiceFactory {
    pub async fn get(&self) -> HttpService {
        let mut factory = self.factory.lock().await;
        factory.make_service(()).await.expect("Expected Infallible")
    }

    pub async fn with_layer<L, S, E>(&self, layer: L) -> HttpServiceFactory
    where
        L: Layer<HttpService, Service = S>,
        S: Service<http::Request<Full<Bytes>>, Response = http::Response<Bytes>, Error = E>
            + Clone
            + Send
            + 'static,
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

#[derive(Clone, Debug, Builder, Default, Getters)]
pub struct HttpServiceConfig {
    accept_invalid_certificates: Option<bool>,
    accept_invalid_hostnames: Option<bool>,
    timeout: Option<Duration>,
}

impl TryFrom<&HttpServiceConfig> for HttpsConnector<HttpConnector> {
    type Error = HttpServiceError;
    fn try_from(value: &HttpServiceConfig) -> Result<Self, Self::Error> {
        let accept_invalid_certificates = value.accept_invalid_certificates.unwrap_or_default();
        let accept_invalid_hostnames = value.accept_invalid_hostnames.unwrap_or_default();
        if accept_invalid_certificates || accept_invalid_hostnames {
            Ok(crate::dangerous::connector(
                accept_invalid_certificates,
                accept_invalid_hostnames,
            ))
        } else {
            Ok(HttpsConnector::<HttpConnector>::builder()
                .with_native_roots()
                .map_err(|err| HttpServiceError::NoCACerts(Box::new(err)))?
                .https_or_http()
                .enable_http1()
                .build())
        }
    }
}

#[derive(Clone, Debug)]
pub struct HyperService {
    client: BoxCloneService<
        http::Request<Full<Bytes>>,
        http::Response<
            ResponseBody<DecompressionBody<Incoming>, NeverClassifyEos<StatusInRangeFailureClass>>,
        >,
        HttpServiceError,
    >,
}

#[buildstructor]
impl HyperService {
    pub fn new(config: &HttpServiceConfig) -> Result<HyperService, HttpServiceError> {
        HyperService::builder()
            .connector((config).try_into()?)
            .and_timeout(*config.timeout())
            .build()
    }
    #[builder(entry = "builder")]
    pub fn do_builder(
        connector: Option<HttpsConnector<HttpConnector>>,
        timeout: Option<Duration>,
    ) -> Result<HyperService, HttpServiceError> {
        let connector = match connector {
            Some(connector) => connector,
            None => HttpsConnector::<HttpConnector>::builder()
                .with_native_roots()
                .map_err(|err| HttpServiceError::NoCACerts(Box::new(err)))?
                .https_or_http()
                .enable_http1()
                .build(),
        };
        let client = Client::builder(TokioExecutor::new()).build(connector);
        let service = HyperService {
            client: ServiceBuilder::new()
                .timeout(timeout.unwrap_or_else(|| Duration::from_secs(90)))
                .layer(TraceLayer::new(
                    StatusInRangeAsFailures::new_for_client_and_server_errors()
                        .into_make_classifier(),
                ))
                .layer(FollowRedirectLayer::new())
                .layer(DecompressionLayer::new().br(true).gzip(true))
                .service(client)
                .map_err(HttpServiceError::from)
                .boxed_clone(),
        };
        Ok(service)
    }
}

impl Service<http::Request<Full<Bytes>>> for HyperService {
    type Response = http::Response<Bytes>;
    type Error = HttpServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<Full<Bytes>>) -> Self::Future {
        let client = self.client.clone();
        let mut client = std::mem::replace(&mut self.client, client);
        let fut = async move {
            let mut resp = client.call(req).await.map_err(|err| {
                if err.is_connect() {
                    HttpServiceError::Connect(Box::new(err))
                } else {
                    HttpServiceError::Unexpected(Box::new(err))
                }
            })?;
            let mut bytes = Vec::new();
            while let Some(next) = resp.frame().await {
                let frame = next?;
                if let Some(chunk) = frame.data_ref() {
                    bytes.extend_from_slice(chunk);
                }
            }
            let bytes = Bytes::from(bytes);
            let resp = resp.map(move |_| bytes.clone());
            Ok(resp)
        };
        Box::pin(fut)
    }
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

impl From<hyper::Error> for HttpServiceError {
    fn from(value: hyper::Error) -> Self {
        if value.is_canceled() {
            HttpServiceError::Cancelled(Box::new(value))
        } else if value.is_closed() {
            HttpServiceError::Closed(Box::new(value))
        } else if value.is_incomplete_message() {
            HttpServiceError::Incomplete(Box::new(value))
        } else if value.is_timeout() {
            HttpServiceError::TimedOut(Box::new(value))
        } else if value.is_body_write_aborted() || value.is_parse() {
            HttpServiceError::Body(Box::new(value))
        } else {
            HttpServiceError::Unexpected(Box::new(value))
        }
    }
}

impl From<Utf8Error> for HttpServiceError {
    fn from(value: Utf8Error) -> Self {
        HttpServiceError::Decode(Box::new(value))
    }
}

impl From<HyperService> for HttpServiceFactory {
    fn from(value: HyperService) -> Self {
        HttpServiceFactory::from(HttpService::from(value))
    }
}

impl From<HyperService> for HttpService {
    fn from(value: HyperService) -> Self {
        value.boxed_clone()
    }
}

impl From<HttpService> for HttpServiceFactory {
    fn from(value: HttpService) -> Self {
        HttpServiceFactory {
            factory: Arc::new(Mutex::new(Shared::new(value))),
        }
    }
}
