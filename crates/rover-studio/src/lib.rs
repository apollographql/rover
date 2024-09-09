use std::{future::Future, pin::Pin};

use buildstructor::buildstructor;
use bytes::Bytes;
use houston::Credential;
use http::{HeaderMap, HeaderValue};
use http_body_util::Full;
use rover_http::HttpServiceError;
use tower::{Layer, Service};

const CLIENT_NAME: &str = "rover-client";

pub struct HttpStudioServiceLayer {
    headers: HeaderMap,
}

#[buildstructor]
impl HttpStudioServiceLayer {
    #[builder]
    pub fn new(
        credential: Credential,
        client_version: String,
        is_sudo: bool,
    ) -> Result<HttpStudioServiceLayer, http::header::InvalidHeaderValue> {
        let mut headers = HeaderMap::new();

        // The headers "apollographql-client-name" and "apollographql-client-version"
        // are used for client identification in Apollo Studio.

        // This provides metrics in Studio that help keep track of what parts of the schema
        // Rover uses, which ensures future changes to the API do not break Rover users.
        // more info here:
        // https://www.apollographql.com/docs/studio/client-awareness/#using-apollo-server-and-apollo-client
        let client_name = HeaderValue::from_static(CLIENT_NAME);
        headers.insert("apollographql-client-name", client_name);
        tracing::debug!(?client_version);
        let client_version = HeaderValue::from_str(&client_version)?;
        headers.insert("apollographql-client-version", client_version);

        let mut api_key = HeaderValue::from_str(&credential.api_key)?;
        api_key.set_sensitive(true);
        headers.insert("x-api-key", api_key);

        if is_sudo {
            headers.insert("apollo-sudo", HeaderValue::from_static("true"));
        }
        Ok(HttpStudioServiceLayer { headers })
    }
}

impl<S> Layer<S> for HttpStudioServiceLayer {
    type Service = HttpStudioService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HttpStudioService {
            headers: self.headers.clone(),
            inner,
        }
    }
}

#[derive(Clone)]
pub struct HttpStudioService<S> {
    headers: HeaderMap,
    inner: S,
}

impl<S> Service<http::Request<Full<Bytes>>> for HttpStudioService<S>
where
    S: Service<
            http::Request<Full<Bytes>>,
            Response = http::Response<Bytes>,
            Error = HttpServiceError,
        > + 'static,
    S::Future: Send,
{
    type Response = http::Response<Bytes>;
    type Error = HttpServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<Full<Bytes>>) -> Self::Future {
        let headers = req.headers_mut();
        headers.extend(self.headers.clone());
        Box::pin(self.inner.call(req))
    }
}
