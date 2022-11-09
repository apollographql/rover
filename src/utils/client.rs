use core::fmt;
use std::{io, str::FromStr, time::Duration};

use crate::{options::ProfileOpt, PKG_NAME, PKG_VERSION};
use anyhow::Result;

use houston as config;
use reqwest::blocking::Client;
use rover_client::blocking::StudioClient;

use serde::Serialize;

/// the Apollo graph registry's production API endpoint
const STUDIO_PROD_API_ENDPOINT: &str = "https://api.apollographql.com/graphql";

#[derive(Debug, Clone, Copy)]
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
        let client = Client::builder()
            .gzip(true)
            .brotli(true)
            .danger_accept_invalid_certs(self.accept_invalid_certs)
            .danger_accept_invalid_hostnames(self.accept_invalid_hostnames)
            .timeout(self.timeout)
            .user_agent(format!("{}/{}", PKG_NAME, PKG_VERSION))
            .build()?;

        Ok(client)
    }
}

#[derive(Debug, Copy, Clone, Serialize)]
pub(crate) struct ClientTimeout {
    duration: Duration,
}

impl ClientTimeout {
    pub(crate) fn new(duration_in_seconds: u64) -> ClientTimeout {
        ClientTimeout {
            duration: Duration::from_secs(duration_in_seconds),
        }
    }

    pub(crate) fn get_duration(&self) -> Duration {
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

#[derive(Debug, Clone)]
pub struct StudioClientConfig {
    pub(crate) config: config::Config,
    client_builder: ClientBuilder,
    uri: String,
    version: String,
    is_sudo: bool,
    client: Option<Client>,
}

impl StudioClientConfig {
    pub fn new(
        override_endpoint: Option<String>,
        config: config::Config,
        is_sudo: bool,
        client_builder: ClientBuilder,
    ) -> StudioClientConfig {
        let version = if cfg!(debug_assertions) {
            format!("{} (dev)", PKG_VERSION)
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

    #[cfg(feature = "composition-js")]
    pub(crate) fn get_builder(&self) -> ClientBuilder {
        self.client_builder
    }

    pub fn get_authenticated_client(&self, profile_opt: &ProfileOpt) -> Result<StudioClient> {
        let credential = config::Profile::get_credential(&profile_opt.profile_name, &self.config)?;
        Ok(StudioClient::new(
            credential,
            &self.uri,
            &self.version,
            self.is_sudo,
            self.get_reqwest_client()?,
        ))
    }
}
