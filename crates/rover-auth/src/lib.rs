//! OAuth2 authentication services for rover.

#![warn(missing_docs)]

use std::{marker::PhantomData, sync::Arc};

use ::oauth2::AsyncHttpClient;
use http::{Request, Response};
use rover_http::{Body, BodyExt};
use rover_tower::ResponseFuture;
use tokio::sync::Mutex;
use tower::Service;

/// Core OAuth2 service types for rover.
pub mod oauth2;

/// Tower [`Service`] wrapper that implements [`oauth2::AsyncHttpClient`].
#[derive(Clone)]
pub(crate) struct OauthHttpClient<T, B> {
    inner: Arc<Mutex<T>>,
    _body: PhantomData<B>,
}

impl<T, B> OauthHttpClient<T, B> {
    /// Creates a new [`OauthHttpClient`] wrapping the given tower service.
    pub(crate) fn new(inner: T) -> OauthHttpClient<T, B> {
        OauthHttpClient {
            inner: Arc::new(Mutex::new(inner)),
            _body: PhantomData,
        }
    }
}

impl<'c, T, B> AsyncHttpClient<'c> for OauthHttpClient<T, B>
where
    T: Service<Request<B>, Response = Response<B>> + Send + 'static,
    T::Error: std::error::Error + From<B::Error> + 'static,
    T::Future: Send,
    B: From<Vec<u8>> + Body + Unpin + Send,
    B::Data: Send,
{
    type Error = T::Error;
    type Future = ResponseFuture<Result<::oauth2::HttpResponse, Self::Error>>;

    fn call(&'c self, request: ::oauth2::HttpRequest) -> Self::Future {
        let service = self.inner.clone();
        let fut = async move {
            let mut service = service.lock().await;
            let (parts, body) = request.into_parts();
            let body = B::from(body);
            let request = Request::from_parts(parts, body);
            let resp = service.call(request).await?;
            let (parts, body) = resp.into_parts();
            let body = body.collect().await?;
            let body = body.to_bytes().to_vec();
            Ok(Response::from_parts(parts, body))
        };
        Box::pin(fut)
    }
}
