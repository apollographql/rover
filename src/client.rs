use anyhow::Result;
use houston as config;
use rover_client::blocking::StudioClient;
use std::env;

const STUDIO_PROD_API_ENDPOINT: &str = "https://graphql.api.apollographql.com/api/graphql";
// const STUDIO_STAGING_API_ENDPOINT: &str = "https://engine-staging-graphql.apollographql.com/api/graphql";

pub(crate) fn get_studio_client(profile: &str) -> Result<StudioClient> {
    let api_key = config::Profile::get_api_key(profile)?;
    let endpoint =
        env::var("APOLLO_REGISTRY_URI").unwrap_or_else(|_| String::from(STUDIO_PROD_API_ENDPOINT));
    Ok(StudioClient::new(&api_key, &endpoint))
}

// pub(crate) fn get_client(uri: &str) -> Result<Client> {}
