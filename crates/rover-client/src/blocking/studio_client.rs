use crate::{
    blocking::{GraphQLClient, CLIENT_NAME},
    error::EndpointKind,
    RoverClientError,
};

use houston::{Credential, CredentialOrigin};
use std::time::Duration;

use graphql_client::GraphQLQuery;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client as ReqwestClient;

/// Represents a client for making GraphQL requests to Apollo Studio.
pub struct StudioClient {
    pub credential: Credential,
    client: GraphQLClient,
    version: String,
    is_sudo: bool,
}

impl StudioClient {
    /// Construct a new [StudioClient] from an `api_key`, a `uri`, and a `version`.
    /// For use in Rover, the `uri` is usually going to be to Apollo Studio
    pub fn new(
        credential: Credential,
        graphql_endpoint: &str,
        version: &str,
        is_sudo: bool,
        client: ReqwestClient,
        retry_period: Option<Duration>,
    ) -> StudioClient {
        StudioClient {
            credential,
            client: GraphQLClient::new(graphql_endpoint, client, retry_period),
            version: version.to_string(),
            is_sudo,
        }
    }

    /// Client method for making a GraphQL request to Apollo Studio.
    ///
    /// Takes one argument, `variables`. Returns a Response or a RoverClientError.
    /// Automatically retries requests.
    pub async fn post<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let mut header_map = self.build_studio_headers()?;
        self.client
            .post::<Q>(variables, &mut header_map, EndpointKind::ApolloStudio)
            .await
    }

    /// Client method for making a GraphQL request to Apollo Studio.
    ///
    /// Takes one argument, `variables`. Returns a Response or a RoverClientError.
    /// Does not automatically retry requests.
    pub async fn post_no_retry<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let mut header_map = self.build_studio_headers()?;
        self.client
            .post_no_retry::<Q>(variables, &mut header_map, EndpointKind::ApolloStudio)
            .await
    }

    /// Function for building a [HeaderMap] for making http requests. Use for making
    /// requests to Apollo Studio. We're leaving this separate from `build` since we
    /// need to be able to mark the api_key as sensitive (at the bottom)
    ///
    /// Takes an `api_key` and a `client_version`, and returns a [HeaderMap].
    pub fn build_studio_headers(&self) -> Result<HeaderMap, RoverClientError> {
        let mut headers = HeaderMap::new();

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

        if self.is_sudo {
            headers.insert("apollo-sudo", HeaderValue::from_str("true")?);
        }

        Ok(headers)
    }

    pub fn get_credential_origin(&self) -> CredentialOrigin {
        self.credential.origin.clone()
    }
}
