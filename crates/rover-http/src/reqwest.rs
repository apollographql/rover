use std::{pin::Pin, time::Duration};

use buildstructor::buildstructor;
use futures::Future;
use http_body_util::Full;
use reqwest::ClientBuilder;
use tower::{Service, ServiceBuilder, ServiceExt, util::BoxCloneService};

use crate::{
    HttpRequest, HttpResponse, HttpService, HttpServiceConfig, HttpServiceError,
    HttpServiceFactory, body::body_to_bytes,
};

/// Constructs [`HttpService`]s
#[derive(Clone, Debug)]
pub struct ReqwestServiceFactory {
    config: HttpServiceConfig,
    client: reqwest::Client,
}

impl HttpServiceFactory for ReqwestServiceFactory {
    /// Creates a new [`HttpService`]
    fn create(&self) -> Result<HttpService, HttpServiceError> {
        let service = ReqwestService::builder()
            .config(self.config.clone())
            .client(self.client.clone())
            .build()
            .map_err(HttpServiceError::from)?;
        Ok(service.boxed_clone())
    }
}

/// A [`Service`] that wraps a [`reqwest`] client and uses [`http`] constructs for requests and responses
#[derive(Clone, Debug)]
pub struct ReqwestService {
    client: BoxCloneService<reqwest::Request, reqwest::Response, HttpServiceError>,
}

#[buildstructor]
impl ReqwestService {
    /// Constructs a new [`ReqwestService`]
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
        let client = ServiceBuilder::new()
            .map_err(HttpServiceError::from)
            .timeout((*config.timeout()).unwrap_or_else(|| Duration::from_secs(90)))
            .service(client)
            .boxed_clone();
        Ok(ReqwestService { client })
    }
}

impl From<reqwest::Error> for HttpServiceError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_body() {
            HttpServiceError::Body(value.into())
        } else if value.is_connect() {
            HttpServiceError::Connect(value.into())
        } else if value.is_timeout() {
            HttpServiceError::TimedOut
        } else {
            HttpServiceError::Unexpected(value.into())
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
        self.client.poll_ready(cx).map_err(HttpServiceError::from)
    }

    fn call(&mut self, req: HttpRequest) -> Self::Future {
        // https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services
        let mut client = self.client.clone();
        let fut = async move {
            let mut req = req.clone();
            let bytes = body_to_bytes(&mut req)
                .await
                .map_err(|err| HttpServiceError::Body(Box::new(err)))?;
            let body = reqwest::Body::from(bytes);
            let req = req.map(move |_| body);
            let req = reqwest::Request::try_from(req)?;
            let mut resp = http::Response::from(client.call(req).await?);
            let bytes = body_to_bytes(&mut resp)
                .await
                .map_err(|err| HttpServiceError::Body(Box::new(err)))?;
            Ok(resp.map(|_| Full::new(bytes)))
        };
        Box::pin(fut)
    }
}

impl From<ReqwestService> for HttpService {
    fn from(value: ReqwestService) -> Self {
        value.boxed_clone()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Result;
    use bytes::Bytes;
    use http::HeaderValue;
    use http_body_util::Full;
    use httpmock::{Method, MockServer};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::{Service, ServiceExt};

    use crate::{HttpService, HttpServiceConfig, HttpServiceError, ReqwestService};

    #[fixture]
    pub fn raw_service() -> HttpService {
        let client = reqwest::Client::default();
        ReqwestService::builder()
            .client(client)
            .build()
            .unwrap()
            .boxed_clone()
    }

    #[fixture]
    pub fn timeout_service() -> HttpService {
        let client = reqwest::Client::default();
        ReqwestService::builder()
            .config(
                HttpServiceConfig::builder()
                    .timeout(Duration::from_millis(100))
                    .build(),
            )
            .client(client)
            .build()
            .unwrap()
            .boxed_clone()
    }

    #[rstest]
    #[case::raw_service(raw_service(), None)]
    #[case::raw_service(timeout_service(), None)]
    #[case::raw_service(timeout_service(), Some(Duration::from_millis(200)))]
    #[tokio::test]
    pub async fn make_a_request(
        #[case] mut service: HttpService,
        #[case] request_length: Option<Duration>,
    ) -> Result<()> {
        let server = MockServer::start();
        let addr = server.address().to_string();
        let uri = format!("http://{}", addr);

        let mock = server.mock(|when, then| {
            when.method(Method::POST)
                .path("/")
                .header("x-some-header", "x-some-value")
                .body("abc");

            let then = then
                .status(200)
                .header("x-resp-header", "x-resp-value")
                .body("def");
            if let Some(request_length) = request_length {
                then.delay(request_length);
            }
        });

        let request = http::Request::builder()
            .uri(uri)
            .method(http::Method::POST)
            .header("x-some-header", "x-some-value")
            .body(Full::new(Bytes::from("abc".as_bytes())))?;

        let resp = service.call(request).await;

        mock.assert_calls(1);

        if request_length.is_some() {
            assert_that!(resp)
                .is_err()
                .matches(|err| matches!(err, HttpServiceError::TimedOut(_)));
        } else {
            let resp = resp?;
            assert_that!(resp.headers().get("x-resp-header"))
                .is_some()
                .is_equal_to(&HeaderValue::from_static("x-resp-value"));

            assert_that!(resp.body()).is_equal_to(&Bytes::from("def".as_bytes()));
        }

        Ok(())
    }
}
