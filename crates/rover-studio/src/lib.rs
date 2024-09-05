use buildstructor::buildstructor;
use houston::Credential;
use http::{HeaderMap, HeaderValue};
use rover_http::extend_headers::ExtendHeaders;
use tower::Layer;

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

impl<S: Clone> Layer<S> for HttpStudioServiceLayer {
    type Service = ExtendHeaders<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ExtendHeaders::new(self.headers.clone(), inner)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use bytes::Bytes;
    use houston::{Credential, CredentialOrigin};
    use http::{HeaderValue, Method, StatusCode};
    use http_body_util::Full;
    use rover_http::{HttpRequest, HttpResponse, HttpServiceError};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tokio::task;
    use tokio_test::assert_ready_ok;
    use tower::ServiceBuilder;
    use tower_test::mock::{self, Mock};

    use crate::HttpStudioServiceLayer;

    #[fixture]
    fn credential() -> Credential {
        Credential {
            api_key: "api_key".to_string(),
            origin: CredentialOrigin::EnvVar,
        }
    }

    #[fixture]
    fn client_version() -> String {
        "client_version".to_string()
    }

    #[rstest]
    #[case::is_sudo(true)]
    #[case::is_not_sudo(false)]
    #[tokio::test]
    pub async fn test_studio_layer(
        credential: Credential,
        client_version: String,
        #[case] is_sudo: bool,
    ) -> Result<()> {
        let expected_client_version_header = HeaderValue::from_str(&client_version)?;
        let (mut service, mut handle) =
            mock::spawn_with(move |inner: Mock<HttpRequest, HttpResponse>| {
                ServiceBuilder::new()
                    .layer(
                        HttpStudioServiceLayer::new(
                            credential.clone(),
                            client_version.to_string(),
                            is_sudo,
                        )
                        .unwrap(),
                    )
                    .map_err(HttpServiceError::Unexpected)
                    .service(inner)
            });
        assert_ready_ok!(service.poll_ready());

        let req = http::Request::builder()
            .uri("https://example.com")
            .method(Method::POST)
            .body(Full::default())?;

        let service_call_fut = task::spawn(service.call(req));
        task::spawn(async move {
            let (actual, send_response) = match handle.next_request().await {
                Some(r) => r,
                None => panic!("expected a request but none was received."),
            };
            let headers = actual.headers();
            assert_that!(headers.get("apollographql-client-version"))
                .is_some()
                .is_equal_to(&expected_client_version_header);
            assert_that!(headers.get("apollographql-client-name"))
                .is_some()
                .is_equal_to(&HeaderValue::from_static("rover-client"));
            assert_that!(headers.get("x-api-key"))
                .is_some()
                .is_equal_to(&HeaderValue::from_static("api_key"));
            if is_sudo {
                assert_that!(headers.get("apollo-sudo"))
                    .is_some()
                    .is_equal_to(&HeaderValue::from_static("true"));
            }
            let resp = http::Response::builder()
                .status(StatusCode::CREATED)
                .body(Bytes::default())
                .unwrap();
            send_response.send_response(resp);
        });

        let result = service_call_fut.await?;
        assert_that!(result)
            .is_ok()
            .matches(|req| req.status() == StatusCode::CREATED);
        Ok(())
    }
}
