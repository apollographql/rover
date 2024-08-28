use http::HeaderMap;
use tower::{Layer, Service};

pub struct ExtendHeadersLayer {
    headers: HeaderMap,
}

impl ExtendHeadersLayer {
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

#[derive(Clone)]
pub struct ExtendHeaders<S: Clone> {
    headers: HeaderMap,
    inner: S,
}

impl<S: Clone> ExtendHeaders<S> {
    pub fn new(headers: HeaderMap, inner: S) -> ExtendHeaders<S> {
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
