use crate::Result;
use crate::PKG_VERSION;

use houston as config;
use reqwest::blocking::Client;
use rover_client::blocking::StudioClient;

/// the Apollo graph registry's production API endpoint
const STUDIO_PROD_API_ENDPOINT: &str = "https://graphql.api.apollographql.com/api/graphql";

pub(crate) fn get_configured_client(
    accept_invalid_certs: bool,
    accept_invalid_hostnames: bool,
) -> Result<Client> {
    let client = Client::builder()
        .gzip(true)
        .brotli(true)
        .danger_accept_invalid_certs(accept_invalid_certs)
        .danger_accept_invalid_hostnames(accept_invalid_hostnames)
        .build()?;
    Ok(client)
}

pub struct StudioClientConfig {
    pub(crate) config: config::Config,
    client: Client,
    uri: String,
    version: String,
    is_sudo: bool,
}

impl StudioClientConfig {
    pub fn new(
        override_endpoint: Option<String>,
        config: config::Config,
        is_sudo: bool,
        client: Client,
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
            client,
            is_sudo,
        }
    }

    pub(crate) fn get_reqwest_client(&self) -> Client {
        // we can use clone here freely since `reqwest` uses an `Arc` under the hood
        self.client.clone()
    }

    pub fn get_authenticated_client(&self, profile_name: &str) -> Result<StudioClient> {
        let credential = config::Profile::get_credential(profile_name, &self.config)?;
        Ok(StudioClient::new(
            credential,
            &self.uri,
            &self.version,
            self.is_sudo,
            self.get_reqwest_client(),
        )?)
    }
}
