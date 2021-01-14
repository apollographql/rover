use crate::blocking::Client;
use crate::headers;
use graphql_client::GraphQLQuery;
use rover_error::RoverError;

/// Represents a client for making GraphQL requests to Apollo Studio.
pub struct StudioClient {
    api_key: String,
    client: reqwest::blocking::Client,
    uri: String,
}

impl StudioClient {
    /// Construct a new [StudioClient] from 2 strings, an `api_key` and a `uri`.
    /// For use in Rover, the `uri` is usually going to be to Apollo Studio
    pub fn new(api_key: &str, uri: &str) -> StudioClient {
        StudioClient {
            api_key: api_key.to_string(),
            client: reqwest::blocking::Client::new(),
            uri: uri.to_string(),
        }
    }

    /// Client method for making a GraphQL request.
    ///
    /// Takes one argument, `variables`. Returns an optional response.
    pub fn post<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, RoverError> {
        let h = headers::build_studio_headers(&self.api_key)?;
        let body = Q::build_query(variables);
        let response = self.client.post(&self.uri).headers(h).json(&body).send()?;
        Client::handle_response::<Q>(response)
    }
}
