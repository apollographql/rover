use std::{future::Future, pin::Pin};

use reqwest::Client;
use serde::Deserialize;
use tower::{util::BoxCloneService, Service};

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

#[derive(Debug, Clone)]
pub struct GitHubService {
    client: Client,
    base_url: String,
}

impl Default for GitHubService {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api.github.com".to_string(),
        }
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GetTarRequest {
    pub owner: String,
    pub repo: String,
    pub reference: String,
}

impl GetTarRequest {
    pub fn new(owner: String, repo: String, reference: String) -> Self {
        Self {
            owner,
            repo,
            reference,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub name: String,
    pub tag_name: String,
    pub html_url: String,
    pub tarball_url: String,
    pub zipball_url: String,
}

impl Service<GetTarRequest> for GitHubService {
    type Response = Vec<u8>;
    type Error = GitHubServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: GetTarRequest) -> Self::Future {
        let url = format!(
            "{}/repos/{}/{}/tarball/{}",
            self.base_url, req.owner, req.repo, req.reference
        );
        let client = self.client.clone();

        Box::pin(async move {
            let response = client
                .get(&url)
                .header("User-Agent", format!("rover-client/{PKG_VERSION}"))
                .send()
                .await
                .map_err(|e| GitHubServiceError::ClientError(e.to_string()))?;

            if !response.status().is_success() {
                return Err(GitHubServiceError::ClientError(format!(
                    "GitHub API request failed with status: {}",
                    response.status()
                )));
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|e| GitHubServiceError::ClientError(e.to_string()))?;

            Ok(bytes.to_vec())
        })
    }
}

#[derive(Debug, Clone)]
pub struct GetAllReleasesRequest {
    pub owner: String,
    pub repo: String,
}

impl GetAllReleasesRequest {
    pub fn new(owner: String, repo: String) -> Self {
        Self { owner, repo }
    }
}

impl Service<GetAllReleasesRequest> for GitHubService {
    type Response = Vec<Release>;
    type Error = GitHubServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: GetAllReleasesRequest) -> Self::Future {
        let url = format!(
            "{}/repos/{}/{}/releases",
            self.base_url, req.owner, req.repo
        );
        let client = self.client.clone();

        Box::pin(async move {
            let response = client
                .get(&url)
                .header("User-Agent", format!("rover-client/{PKG_VERSION}"))
                .send()
                .await
                .map_err(|e| GitHubServiceError::ClientError(e.to_string()))?;

            if !response.status().is_success() {
                return Err(GitHubServiceError::ClientError(format!(
                    "GitHub API request failed with status: {}",
                    response.status()
                )));
            }

            let releases: Vec<Release> = response
                .json()
                .await
                .map_err(|e| GitHubServiceError::ClientError(e.to_string()))?;

            Ok(releases)
        })
    }
}

pub type BoxedGitHubService =
    BoxCloneService<GetAllReleasesRequest, Vec<Release>, GitHubServiceError>;

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_tar() {
        let mut service = GitHubService::new();
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

    #[tokio::test]
    async fn test_get_all_releases() {
        let mut service = GitHubService::new();
        let request = GetAllReleasesRequest::new(
            "apollographql".to_string(),
            "rover-init-starters".to_string(),
        );

        let ready_service = ServiceExt::<GetAllReleasesRequest>::ready(&mut service)
            .await
            .unwrap();
        let result: Result<Vec<Release>, GitHubServiceError> = ready_service.call(request).await;
        assert!(result.is_ok());
    }
}
