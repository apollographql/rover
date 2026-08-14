use std::{fmt, future::Future, pin::Pin, time::Duration};

use apollo_http_client::{HttpClient, HttpClientConfig};
use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use tower::{util::BoxCloneService, BoxError, Service, ServiceBuilder, ServiceExt};

use crate::error::RoverClientError;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(thiserror::Error, Debug)]
pub enum GitHubServiceError {
    #[error("Service failed to reach a ready state.\n{}", .0)]
    ServiceReady(Box<dyn std::error::Error + Send + Sync>),
    #[error("GitHub API request failed: {}", .0)]
    ClientError(String),
}

impl From<GitHubServiceError> for RoverClientError {
    fn from(value: GitHubServiceError) -> Self {
        match value {
            GitHubServiceError::ServiceReady(err) => RoverClientError::ServiceReady(err),
            GitHubServiceError::ClientError(msg) => RoverClientError::ClientError { msg },
        }
    }
}

type HttpRequest = http::Request<Empty<Bytes>>;
type HttpResponse = <HttpClient as Service<HttpRequest>>::Response;

/// The inner, request-dispatching service backing a [`GitHubService`]
type GitHubHttpService = BoxCloneService<HttpRequest, HttpResponse, GitHubServiceError>;

/// Tower [`Service`] that sends requests to the GitHub REST API.
///
/// Constructed via [`GitHubService::builder`]. All clones share the same
/// underlying connection pool.
#[derive(Clone)]
pub struct GitHubService {
    client: GitHubHttpService,
    base_url: String,
}

impl fmt::Debug for GitHubService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GitHubService")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

#[bon::bon]
impl GitHubService {
    #[builder]
    pub fn new(
        #[builder(default = "https://api.github.com".to_string())] base_url: String,
        #[builder(default)] accept_invalid_certs: bool,
        #[builder(default = Duration::from_secs(30))] timeout: Duration,
    ) -> Self {
        let mut config = HttpClientConfig::default();
        config.tls.danger_accept_invalid_certs = accept_invalid_certs;
        let client = HttpClient::new(&config).expect("Failed to build HTTP client");
        let client = ServiceBuilder::new().timeout(timeout).service(client);
        let client = ServiceExt::<HttpRequest>::map_err(client, |err: BoxError| {
            GitHubServiceError::ClientError(err.to_string())
        });
        let client: GitHubHttpService = ServiceExt::<HttpRequest>::boxed_clone(client);
        Self { client, base_url }
    }
}

#[derive(Debug, Clone)]
pub struct GetTarRequest {
    pub owner: String,
    pub repo: String,
    pub reference: String,
}

impl GetTarRequest {
    pub const fn new(owner: String, repo: String, reference: String) -> Self {
        Self {
            owner,
            repo,
            reference,
        }
    }
}

/// Sends a single GET request through `client` and returns the response.
async fn dispatch(
    client: GitHubHttpService,
    request: http::Request<Empty<Bytes>>,
) -> Result<HttpResponse, GitHubServiceError> {
    client.oneshot(request).await
}

/// Builds a `GET` request to `uri` with the rover `User-Agent` header.
fn build_get_request(
    uri: impl AsRef<str>,
) -> Result<http::Request<Empty<Bytes>>, GitHubServiceError> {
    http::Request::builder()
        .method(http::Method::GET)
        .uri(uri.as_ref())
        .header(
            http::header::USER_AGENT,
            format!("rover-client/{PKG_VERSION}"),
        )
        .body(Empty::<Bytes>::new())
        .map_err(|e| GitHubServiceError::ClientError(e.to_string()))
}

impl Service<GetTarRequest> for GitHubService {
    type Response = Vec<u8>;
    type Error = GitHubServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.client.poll_ready(cx)
    }

    fn call(&mut self, req: GetTarRequest) -> Self::Future {
        let url = format!(
            "{}/repos/{}/{}/tarball/{}",
            self.base_url, req.owner, req.repo, req.reference
        );
        let client = self.client.clone();

        Box::pin(async move {
            let request = build_get_request(&url)?;
            let response = dispatch(client.clone(), request).await?;

            // GitHub's tarball endpoint issues a 302 redirect to S3/CDN.
            // Follow at most one hop.
            let response = if response.status().is_redirection() {
                let location = response
                    .headers()
                    .get(http::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| {
                        GitHubServiceError::ClientError(
                            "redirect with missing or non-UTF-8 Location header".to_string(),
                        )
                    })?
                    .to_owned();

                dispatch(client, build_get_request(location)?).await?
            } else {
                response
            };

            if !response.status().is_success() {
                return Err(GitHubServiceError::ClientError(format!(
                    "GitHub API request failed with status: {}",
                    response.status()
                )));
            }

            let bytes = response
                .into_body()
                .collect()
                .await
                .map_err(|e| GitHubServiceError::ClientError(e.to_string()))?
                .to_bytes();

            Ok(bytes.to_vec())
        })
    }
}

#[cfg(test)]
mod tests {
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn test_get_tar() {
        let mut service = GitHubService::builder().build();
        let request = GetTarRequest::new(
            "apollographql".to_string(),
            "rover-init-starters".to_string(),
            "v0.1.0".to_string(),
        );

        let ready_service = ServiceExt::<GetTarRequest>::ready(&mut service)
            .await
            .unwrap();
        let result: Result<Vec<u8>, GitHubServiceError> = ready_service.call(request).await;
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
    }
}
