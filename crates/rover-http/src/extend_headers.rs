//! Provides utilities to uniformly add headers to HTTP requests

use http::HeaderMap;
use tower::{Layer, Service};

/// Layer that applies [`ExtendHeaders`], which adds headers to HTTP requests
pub struct ExtendHeadersLayer {
    headers: HeaderMap,
}

impl ExtendHeadersLayer {
    /// Constructs a new [`ExtendHeadersLayer`]
    pub fn new(headers: impl Into<HeaderMap>) -> ExtendHeadersLayer {
        ExtendHeadersLayer {
            headers: headers.into(),
        }
    }
}

impl<S: Clone> Layer<S> for ExtendHeadersLayer {
    type Service = ExtendHeaders<S>;
    fn layer(&self, inner: S) -> Self::Service {
        ExtendHeaders {
            headers: self.headers.clone(),
            inner,
        }
    }
}

/// Middleware that adds headers to HTTP requests
#[derive(Clone)]
pub struct ExtendHeaders<S: Clone> {
    headers: HeaderMap,
    inner: S,
}

impl<S: Clone> ExtendHeaders<S> {
    /// Constructs a new [`ExtendHeaders`]
    pub const fn new(headers: HeaderMap, inner: S) -> ExtendHeaders<S> {
        ExtendHeaders { headers, inner }
    }
}

impl<Req, S> Service<http::Request<Req>> for ExtendHeaders<S>
where
    S: Service<http::Request<Req>> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<Req>) -> Self::Future {
        req.headers_mut().extend(self.headers.clone());
        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use http::{HeaderMap, HeaderName, HeaderValue};
    use http_body_util::Full;
    use httpmock::MockServer;
    use rstest::{fixture, rstest};
    use tower::{Service, ServiceBuilder, ServiceExt};

    use crate::{HttpService, ReqwestService};

    use super::ExtendHeadersLayer;

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
    pub fn extend_headers_service(raw_service: HttpService) -> HttpService {
        ServiceBuilder::new()
            .layer(ExtendHeadersLayer::new(HeaderMap::from_iter([(
                HeaderName::from_static("x-custom-header"),
                HeaderValue::from_static("x-custom-header-value"),
            )])))
            .service(raw_service)
            .boxed_clone()
    }

    #[rstest]
    #[tokio::test]
    pub async fn test_extend_headers(mut extend_headers_service: HttpService) -> Result<()> {
        let server = MockServer::start();
        let addr = server.address().to_string();
        let uri = format!("http://{}/", addr);

        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/")
                .header("x-original-header", "x-original-header-value")
                .header("x-custom-header", "x-custom-header-value");
            then.status(500).body("");
        });

        let request = http::Request::builder()
            .uri(uri)
            .header("x-original-header", "x-original-header-value")
            .method(http::Method::GET)
            .body(Full::default())?;

        let _ = extend_headers_service.call(request).await?;

        mock.assert_calls(1);

        Ok(())
    }
}
