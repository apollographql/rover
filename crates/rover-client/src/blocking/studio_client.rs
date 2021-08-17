use crate::{
    blocking::{GraphQLClient, CLIENT_NAME, JSON_CONTENT_TYPE},
    RoverClientError,
};

use houston::{Credential, CredentialOrigin};

use graphql_client::GraphQLQuery;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{blocking::Client as ReqwestClient, Error as ReqwestError};

/// Represents a client for making GraphQL requests to Apollo Studio.
pub struct StudioClient {
    credential: Credential,
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
        let header_map = self.build_studio_headers()?;
        let request_body = self.client.get_request_body::<Q>(variables)?;
        let response = self.client.execute(&request_body, header_map)?;
        GraphQLClient::handle_response::<Q>(response)
    }

    /// Function for building a [HeaderMap] for making http requests. Use for making
    /// requests to Apollo Studio. We're leaving this separate from `build` since we
    /// need to be able to mark the api_key as sensitive (at the bottom)
    ///
    /// Takes an `api_key` and a `client_version`, and returns a [HeaderMap].
    pub fn build_studio_headers(&self) -> Result<HeaderMap, RoverClientError> {
        let mut headers = HeaderMap::new();

        let content_type = HeaderValue::from_str(JSON_CONTENT_TYPE)?;
        headers.insert("Content-Type", content_type);

        // The headers "apollographql-client-name" and "apollographql-client-version"
        // are used for client identification in Apollo Studio.

        // This provides metrics in Studio that help keep track of what parts of the schema
        // Rover uses, which ensures future changes to the API do not break Rover users.
        // more info here:
        // https://www.apollographql.com/docs/studio/client-awareness/#using-apollo-server-and-apollo-client

        let client_name = HeaderValue::from_str(CLIENT_NAME)?;
        headers.insert("apollographql-client-name", client_name);
        tracing::debug!(?self.version);
        let client_version = HeaderValue::from_str(&self.version)?;
        headers.insert("apollographql-client-version", client_version);

        let mut api_key = HeaderValue::from_str(&self.credential.api_key)?;
        api_key.set_sensitive(true);
        headers.insert("x-api-key", api_key);

        Ok(headers)
    }

    pub fn get_credential_origin(&self) -> CredentialOrigin {
        self.credential.origin.clone()
    }
}
