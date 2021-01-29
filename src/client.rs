use crate::Result;

use houston as config;
use rover_client::blocking::StudioClient;

const STUDIO_PROD_API_ENDPOINT: &str = "https://graphql.api.apollographql.com/api/graphql";

pub struct StudioClientConfig {
    uri: String,
    config: config::Config,
}

impl StudioClientConfig {
    pub fn new(override_endpoint: Option<String>, config: config::Config) -> StudioClientConfig {
        StudioClientConfig {
            uri: override_endpoint.unwrap_or_else(|| STUDIO_PROD_API_ENDPOINT.to_string()),
            config,
        }
    }

    pub fn get_client(&self, profile_name: &str) -> Result<StudioClient> {
        let api_key = config::Profile::get_api_key(profile_name, &self.config)?;
        Ok(StudioClient::new(&api_key, &self.uri))
    }
}
