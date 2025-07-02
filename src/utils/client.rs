use core::fmt;
use std::{io, str::FromStr, time::Duration};

use anyhow::Result;
use derive_getters::Getters;
use houston as config;
use reqwest::Client;
use rover_client::blocking::StudioClient;
use rover_http::{HttpService, ReqwestService};
use rover_studio::HttpStudioServiceLayer;
use serde::Serialize;
use tower::{ServiceBuilder, ServiceExt};
use url::Url;

use crate::{options::ProfileOpt, PKG_NAME, PKG_VERSION};

/// the Apollo graph registry's production API endpoint
const STUDIO_PROD_API_ENDPOINT: &str = "https://api.apollographql.com/graphql";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClientBuilder {
    accept_invalid_certs: bool,
    accept_invalid_hostnames: bool,
    timeout: Option<std::time::Duration>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self {
            accept_invalid_certs: false,
            accept_invalid_hostnames: false,
            timeout: None,
        }
    }

    pub fn accept_invalid_certs(self, value: bool) -> Self {
        Self {
            accept_invalid_certs: value,
            ..self
        }
    }

    pub fn accept_invalid_hostnames(self, value: bool) -> Self {
        Self {
            accept_invalid_hostnames: value,
            ..self
        }
    }

    pub fn with_timeout(self, timeout: std::time::Duration) -> Self {
        Self {
            timeout: Some(timeout),
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
    pub fn new(duration_in_seconds: u64) -> ClientTimeout {
        ClientTimeout {
            duration: Duration::from_secs(duration_in_seconds),
        }
    }

    pub fn get_duration(&self) -> Duration {
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
        }
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

    pub fn retry_period(&self) -> Duration {
        self.client_timeout.get_duration()
    }
}
