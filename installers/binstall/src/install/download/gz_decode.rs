use std::io::Read;

use bytes::Bytes;
use flate2::read::GzDecoder;
use futures::TryFutureExt;
use rover_http::{Body, BodyExt, HttpRequest, HttpResponse, HttpServiceError};
use rover_tower::ResponseFuture;
use tower::{BoxError, Layer, Service};

#[derive(thiserror::Error, Debug)]
pub enum GzDecodeError {
    #[error(transparent)]
    Upstream(#[from] BoxError),
    #[error(transparent)]
    Http(#[from] HttpServiceError),
    #[error("Failed to decode file: {}", .0)]
    Decode(#[from] std::io::Error),
    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),
}

#[derive(Clone, Debug, Default)]
pub struct GzDecodeLayer {}

impl<S> Layer<S> for GzDecodeLayer {
    type Service = GzDecode<S>;
    fn layer(&self, inner: S) -> Self::Service {
        GzDecode { inner }
    }
}

#[derive(Clone, Debug, Default)]
pub struct GzDecode<S> {
    inner: S,
}

impl<S, T> Service<HttpRequest> for GzDecode<S>
where
    T: Body + Send + Sync + 'static,
    T::Error: Into<GzDecodeError> + 'static,
    T::Data: Send,
    S: Service<HttpRequest, Response = HttpResponse<T>>,
    S::Error: Into<GzDecodeError> + 'static,
    S::Future: Send + 'static,
{
    type Response = Bytes;
    type Error = GzDecodeError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: HttpRequest) -> Self::Future {
        let resp = self
            .inner
            .call(req)
            .map_err(Into::into)
            .and_then(|resp| async move {
                let body = resp
                    .into_body()
                    .collect()
                    .await
                    .map_err(Into::<GzDecodeError>::into)?
                    .to_bytes();
                let mut decoder = GzDecoder::new(&body[..]);
                let mut bytes = Vec::new();
                decoder.read_to_end(&mut bytes)?;
                Ok(Bytes::from(bytes))
            });
        Box::pin(resp)
    }
}
