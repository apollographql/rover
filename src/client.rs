use anyhow::Result;
use houston as config;
use rover_client::blocking::Client;

const STUDIO_PROD_API_ENDPOINT: &str = "https://graphql.api.apollographql.com/api/graphql";

pub(crate) fn get_rover_client(profile: &str) -> Result<Client> {
    let api_key = config::Profile::get_api_key(profile)?;
    Ok(Client::new(&api_key, STUDIO_PROD_API_ENDPOINT))
}
