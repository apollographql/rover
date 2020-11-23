use anyhow::Result;
use houston as config;
use rover_client::blocking::StudioClient;
use std::env;
use url::Url;

pub const STUDIO_PROD_API_ENDPOINT: &str = "https://graphql.api.apollographql.com/api/graphql";
// const STUDIO_STAGING_API_ENDPOINT: &str = "https://engine-staging-graphql.apollographql.com/api/graphql";

pub(crate) fn get_client_endpoint() -> String {
    env::var("APOLLO_REGISTRY_URI").unwrap_or_else(|_| String::from(STUDIO_PROD_API_ENDPOINT))
}

/// this is just the client endpoint with no path. This should be the URL for
/// the UI
pub(crate) fn get_app_url() -> Result<String> {
    let api_endpoint = get_client_endpoint();
    let mut url = Url::parse(&api_endpoint)?;
    url.set_path("");
    Ok(url.to_string())
}

pub(crate) fn get_studio_client(profile: &str) -> Result<StudioClient> {
    let api_key = config::Profile::get_api_key(profile)?;
    let endpoint =
        env::var("APOLLO_REGISTRY_URI").unwrap_or_else(|_| String::from(STUDIO_PROD_API_ENDPOINT));
    Ok(StudioClient::new(&api_key, &endpoint))
}

// pub(crate) fn get_client(uri: &str) -> Result<Client> {}
