use anyhow::Result;
use houston as config;
use rover_client::blocking::StudioClient;

const STUDIO_PROD_API_ENDPOINT: &str = "https://graphql.api.apollographql.com/api/graphql";

pub(crate) fn get_studio_client(profile: &str) -> Result<StudioClient> {
    let api_key = config::Profile::get_api_key(profile)?;
    Ok(StudioClient::new(&api_key, STUDIO_PROD_API_ENDPOINT))
}

// pub(crate) fn get_client(uri: &str) -> Result<Client> {

// }
