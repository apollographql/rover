use crate::Result;

use houston as config;
use rover_client::blocking::StudioClient;

/// the Apollo graph registry's production API endpoint
const STUDIO_PROD_API_ENDPOINT: &str = "https://graphql.api.apollographql.com/api/graphql";

/// the version of Rover currently set in `Cargo.toml`
const ROVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct StudioClientConfig {
    uri: String,
    config: config::Config,
    version: String,
}

impl StudioClientConfig {
    pub fn new(override_endpoint: Option<String>, config: config::Config) -> StudioClientConfig {
        let version = if cfg!(debug_assertions) {
            format!("{} (dev)", ROVER_VERSION)
        } else {
            ROVER_VERSION.to_string()
        };

        StudioClientConfig {
            uri: override_endpoint.unwrap_or_else(|| STUDIO_PROD_API_ENDPOINT.to_string()),
            config,
            version,
        }
    }

    pub fn get_client(&self, profile_name: &str) -> Result<StudioClient> {
        let api_key = config::Profile::get_api_key(profile_name, &self.config)?;
        Ok(StudioClient::new(&api_key, &self.uri, &self.version))
    }
}
