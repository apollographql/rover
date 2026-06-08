use core::fmt;
use std::{io, str::FromStr, time::Duration};

use anyhow::Result;
use derive_getters::Getters;
use houston as config;
use reqwest::Client;
use rover_client::blocking::StudioClient;
use rover_http::{HttpService, ReqwestService};
use rover_studio::service::HttpStudioServiceLayer;
use serde::Serialize;
use tower::{ServiceBuilder, ServiceExt};
use url::Url;

use crate::{PKG_NAME, PKG_VERSION, options::ProfileOpt};

/// the Apollo graph registry's production API endpoint
const STUDIO_PROD_API_ENDPOINT: &str = "https://api.apollographql.com/graphql";

/// How long to wait to establish a connection when downloading a plugin tarball
/// before giving up. If the connection succesfully establishes we allow a much longer
/// period for the actual request.
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientBuilder {
    accept_invalid_certs: bool,
    accept_invalid_hostnames: bool,
    timeout: Option<std::time::Duration>,
    connect_timeout: Option<std::time::Duration>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    pub const fn new() -> Self {
        Self {
            accept_invalid_certs: false,
            accept_invalid_hostnames: false,
            timeout: None,
            connect_timeout: None,
        }
    }

    pub const fn accept_invalid_certs(self, value: bool) -> Self {
        Self {
            accept_invalid_certs: value,
            ..self
        }
    }

    pub const fn accept_invalid_hostnames(self, value: bool) -> Self {
        Self {
            accept_invalid_hostnames: value,
            ..self
        }
    }

    pub const fn with_timeout(self, timeout: std::time::Duration) -> Self {
        Self {
            timeout: Some(timeout),
            ..self
        }
    }

    const fn clear_timeout(self) -> Self {
        Self {
            timeout: None,
            ..self
        }
    }

    const fn with_connect_timeout(self, connect_timeout: std::time::Duration) -> Self {
        Self {
            connect_timeout: Some(connect_timeout),
            ..self
        }
    }

    pub(crate) fn build(self) -> Result<Client> {
        let mut builder = Client::builder()
            .gzip(true)
            .brotli(true)
            .danger_accept_invalid_certs(self.accept_invalid_certs)
            .danger_accept_invalid_hostnames(self.accept_invalid_hostnames);

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(connect_timeout) = self.connect_timeout {
            builder = builder.connect_timeout(connect_timeout);
        }

        let client = builder
            .user_agent(format!("{PKG_NAME}/{PKG_VERSION}"))
            .build()?;

        Ok(client)
    }
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct ClientTimeout {
    duration: Duration,
}

impl ClientTimeout {
    pub const fn new(duration_in_seconds: u64) -> ClientTimeout {
        ClientTimeout {
            duration: Duration::from_secs(duration_in_seconds),
        }
    }

    pub const fn get_duration(&self) -> Duration {
        self.duration
    }
}

impl Default for ClientTimeout {
    fn default() -> ClientTimeout {
        ClientTimeout::new(30)
    }
}

impl FromStr for ClientTimeout {
    type Err = io::Error;
    fn from_str(duration_in_secs: &str) -> std::result::Result<ClientTimeout, io::Error> {
        Ok(ClientTimeout::new(duration_in_secs.parse().map_err(
            |e| io::Error::new(io::ErrorKind::InvalidInput, e),
        )?))
    }
}

impl fmt::Display for ClientTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.duration.as_secs())
    }
}

impl From<Duration> for ClientTimeout {
    fn from(value: Duration) -> Self {
        ClientTimeout::new(value.as_secs())
    }
}

#[derive(Debug, Clone, Getters)]
pub struct StudioClientConfig {
    #[getter(skip)]
    pub(crate) config: config::Config,
    client_builder: ClientBuilder,
    uri: String,
    version: String,
    is_sudo: bool,
    client: Option<Client>,
    client_timeout: ClientTimeout,
    download_timeout: Duration,
}

impl StudioClientConfig {
    pub fn new(
        override_endpoint: Option<String>,
        config: config::Config,
        is_sudo: bool,
        client_builder: ClientBuilder,
        client_timeout: ClientTimeout,
    ) -> StudioClientConfig {
        let version = if cfg!(debug_assertions) {
            format!("{PKG_VERSION} (dev)")
        } else {
            PKG_VERSION.to_string()
        };

        StudioClientConfig {
            uri: override_endpoint.unwrap_or_else(|| STUDIO_PROD_API_ENDPOINT.to_string()),
            config,
            version,
            client_builder,
            is_sudo,
            client: None,
            client_timeout,
            download_timeout: DOWNLOAD_REQUEST_TIMEOUT,
        }
    }

    pub const fn with_download_timeout(mut self, download_timeout: Duration) -> Self {
        self.download_timeout = download_timeout;
        self
    }

    pub(crate) fn get_reqwest_client(&self) -> Result<Client> {
        if let Some(client) = &self.client {
            Ok(client.clone())
        } else {
            // we can use clone here freely since `reqwest` uses an `Arc` under the hood
            self.client_builder.build()
        }
    }

    pub fn service(&self) -> Result<HttpService> {
        let client = self.get_reqwest_client()?;
        Ok(ReqwestService::builder()
            .client(client)
            .build()?
            .boxed_clone())
    }

    /// A service for downloading large binaries (the `supergraph` and `router`
    /// plugins) with a much longer request timeout and a connection timeout to fail-fast
    /// in actual offline scenarios.
    pub fn download_service(&self) -> Result<HttpService> {
        let client = self
            .client_builder
            .clear_timeout()
            .with_timeout(self.download_timeout)
            .with_connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
            .build()?;
        Ok(ReqwestService::builder()
            .client(client)
            .build()?
            .boxed_clone())
    }

    pub fn get_authenticated_client(&self, profile_opt: &ProfileOpt) -> Result<StudioClient> {
        let credential = config::Profile::get_credential(&profile_opt.profile_name, &self.config)?;
        Ok(StudioClient::new(
            credential,
            &self.uri,
            &self.version,
            self.is_sudo,
            self.get_reqwest_client()?,
            self.client_timeout.get_duration(),
        ))
    }

    pub fn authenticated_service(&self, profile_opt: &ProfileOpt) -> Result<HttpService> {
        let client = self.get_reqwest_client()?;
        let credential = config::Profile::get_credential(&profile_opt.profile_name, &self.config)?;
        let service = ServiceBuilder::new()
            .layer(HttpStudioServiceLayer::new(
                Url::from_str(&self.uri)?,
                credential,
                self.version.clone(),
                self.is_sudo,
            )?)
            .service(ReqwestService::builder().client(client).build()?)
            .boxed_clone();
        Ok(service)
    }

    pub const fn accept_invalid_certs(&self) -> bool {
        self.client_builder.accept_invalid_certs
    }

    pub const fn retry_period(&self) -> Duration {
        self.client_timeout.get_duration()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ClientBuilder;

    #[test]
    fn clear_timeout_drops_the_whole_request_deadline() {
        let builder = ClientBuilder::new().with_timeout(Duration::from_secs(30));
        assert_eq!(builder.timeout, Some(Duration::from_secs(30)));
        assert!(builder.clear_timeout().timeout.is_none());
    }

    #[test]
    fn with_connect_timeout_is_independent_of_the_request_timeout() {
        let builder = ClientBuilder::new()
            .with_timeout(Duration::from_secs(30))
            .with_connect_timeout(Duration::from_secs(5));
        assert_eq!(builder.timeout, Some(Duration::from_secs(30)));
        assert_eq!(builder.connect_timeout, Some(Duration::from_secs(5)));
    }

    fn test_client_config() -> super::StudioClientConfig {
        super::StudioClientConfig::new(
            None,
            houston::Config {
                home: camino::Utf8PathBuf::from("/tmp/rover-client-test"),
                override_api_key: None,
            },
            false,
            ClientBuilder::default(),
            super::ClientTimeout::default(),
        )
    }

    /// Plugin downloads default to the generous timeout, and an explicit
    /// `--client-timeout` (applied via `with_download_timeout`) overrides it —
    /// up or down. Regression guard: #3358 made this unconfigurable.
    #[test]
    fn download_timeout_defaults_to_the_generous_default() {
        assert_eq!(
            *test_client_config().download_timeout(),
            super::DOWNLOAD_REQUEST_TIMEOUT
        );
    }

    #[test]
    fn with_download_timeout_overrides_the_default_up_or_down() {
        for secs in [5, 99_999] {
            let config = test_client_config().with_download_timeout(Duration::from_secs(secs));
            assert_eq!(*config.download_timeout(), Duration::from_secs(secs));
        }
    }
}
