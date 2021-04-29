use crate::blocking::Client;
use crate::headers;
use crate::RoverClientError;
use houston::Credential;

use graphql_client::GraphQLQuery;

/// Represents a client for making GraphQL requests to Apollo Studio.
pub struct StudioClient {
    pub credential: Credential,
    client: reqwest::blocking::Client,
    uri: String,
    version: String,
}

impl StudioClient {
    /// Construct a new [StudioClient] from an `api_key`, a `uri`, and a `version`.
    /// For use in Rover, the `uri` is usually going to be to Apollo Studio
    pub fn new(credential: Credential, uri: &str, version: &str) -> StudioClient {
        StudioClient {
            credential,
            client: reqwest::blocking::Client::new(),
            uri: uri.to_string(),
            version: version.to_string(),
        }
    }

    /// Client method for making a GraphQL request.
    ///
    /// Takes one argument, `variables`. Returns an optional response.
    pub fn post<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let h = headers::build_studio_headers(&self.credential.api_key, &self.version)?;
        let body = Q::build_query(variables);
        tracing::trace!(request_headers = ?h);
        tracing::trace!("Request Body: {}", serde_json::to_string(&body)?);

        let response = self
            .client
            .post(&self.uri)
            .headers(h)
            .json(&body)
            .send()?
            .error_for_status()?;
        tracing::trace!(response_status = ?response.status(), response_headers = ?response.headers());

        Client::handle_response::<Q>(response)
    }
}
