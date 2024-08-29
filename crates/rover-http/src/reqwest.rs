use std::{pin::Pin, time::Duration};

use buildstructor::buildstructor;
use bytes::Bytes;
use futures::Future;
use http_body_util::BodyExt;
use reqwest::ClientBuilder;
use tower::{util::BoxCloneService, Service, ServiceBuilder, ServiceExt};
use tower_reqwest::HttpClientLayer;

use crate::{
    HttpRequest, HttpResponse, HttpService, HttpServiceConfig, HttpServiceError, HttpServiceFactory,
};

#[derive(Clone, Debug)]
pub struct ReqwestService {
    service: BoxCloneService<
        http::Request<reqwest::Body>,
        http::Response<reqwest::Body>,
        HttpServiceError,
    >,
}

#[buildstructor]
impl ReqwestService {
    #[builder]
    pub fn new(
        config: Option<HttpServiceConfig>,
        client: Option<reqwest::Client>,
    ) -> Result<ReqwestService, reqwest::Error> {
        let config = config.unwrap_or_default();
        let client = match client {
            Some(client) => client,
            None => ClientBuilder::new()
                .danger_accept_invalid_certs(config.accept_invalid_certificates.unwrap_or_default())
                .danger_accept_invalid_hostnames(
                    config.accept_invalid_hostnames.unwrap_or_default(),
                )
                .build()?,
        };
        let service = ServiceBuilder::new()
            .timeout((*config.timeout()).unwrap_or_else(|| Duration::from_secs(90)))
            .map_err(HttpServiceError::from) // maps from reqwest::Error -> HttpServiceError
            .layer(HttpClientLayer)
            .service(client)
            .map_err(HttpServiceError::from) // maps from timeout's Box<dyn Error> -> HttpServiceError
            .boxed_clone();
        Ok(ReqwestService { service })
    }
}

impl From<tower_reqwest::Error> for HttpServiceError {
    fn from(value: tower_reqwest::Error) -> Self {
        match value {
            tower_reqwest::Error::Client(err) => {
                if err.is_body() {
                    HttpServiceError::Body(err.into())
                } else if err.is_connection() {
                    HttpServiceError::Connect(err.into())
                } else if err.is_timeout() {
                    HttpServiceError::TimedOut(err.into())
                } else {
                    HttpServiceError::Unexpected(err.into())
                }
            }
            tower_reqwest::Error::Middleware(err) => HttpServiceError::Unexpected(err),
        }
    }
}

impl Service<HttpRequest> for ReqwestService {
    type Response = HttpResponse;
    type Error = HttpServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: HttpRequest) -> Self::Future {
        let cloned = self.service.clone();
        let mut service = std::mem::replace(&mut self.service, cloned);
        let fut = async move {
            let mut req = req.clone();
            let mut bytes = Vec::new();
            while let Some(next) = req.frame().await {
                let frame = next.expect("Expected Infallible");
                if let Some(chunk) = frame.data_ref() {
                    bytes.extend_from_slice(chunk);
                }
            }
            let body = reqwest::Body::from(bytes);
            let req = req.map(move |_| body);
            let mut resp = service.call(req).await?;
            let mut bytes = Vec::new();
            while let Some(next) = resp.frame().await {
                let frame = next.expect("Expected Infallible");
                if let Some(chunk) = frame.data_ref() {
                    bytes.extend_from_slice(chunk);
                }
            }
            Ok(resp.map(|_| Bytes::from(bytes)))
        };
        Box::pin(fut)
    }
}

impl From<ReqwestService> for HttpServiceFactory {
    fn from(value: ReqwestService) -> Self {
        HttpServiceFactory::from(HttpService::from(value))
    }
}

impl From<ReqwestService> for HttpService {
    fn from(value: ReqwestService) -> Self {
        value.boxed_clone()
    }
}
