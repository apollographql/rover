use crate::{blocking::GraphQLClient, headers, RoverClientError};
use houston::Credential;

use graphql_client::GraphQLQuery;
use reqwest::{blocking::Client as ReqwestClient, Error as ReqwestError};

/// Represents a client for making GraphQL requests to Apollo Studio.
pub struct StudioClient {
    pub credential: Credential,
    client: GraphQLClient,
    version: String,
}

impl StudioClient {
    /// Construct a new [StudioClient] from an `api_key`, a `uri`, and a `version`.
    /// For use in Rover, the `uri` is usually going to be to Apollo Studio
    pub fn new(
        credential: Credential,
        graphql_endpoint: &str,
        version: &str,
        client: ReqwestClient,
    ) -> Result<StudioClient, ReqwestError> {
        Ok(StudioClient {
            credential,
            client: GraphQLClient::new(graphql_endpoint, client)?,
            version: version.to_string(),
        })
    }

    /// Client method for making a GraphQL request to Apollo Studio.
    ///
    /// Takes one argument, `variables`. Returns a Response or a RoverClientError.
    pub fn post<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let header_map = headers::build_studio_headers(&self.credential.api_key, &self.version)?;
        let response = self.client.execute::<Q>(variables, header_map)?;
        GraphQLClient::handle_response::<Q>(response)
    }
}
