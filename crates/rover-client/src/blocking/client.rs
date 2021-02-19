use crate::{headers, RoverClientError};
use graphql_client::GraphQLQuery;
use reqwest::blocking::{Client as ReqwestClient, Response};
use std::collections::HashMap;

/// Represents a generic GraphQL client for making http requests.
pub struct Client {
    client: ReqwestClient,
    uri: String,
}

impl Client {
    /// Construct a new [Client] from a `uri`.
    /// This client is used for generic GraphQL requests, such as introspection.
    pub fn new(uri: &str) -> Client {
        Client {
            client: ReqwestClient::new(),
            uri: uri.to_string(),
        }
    }

    /// Client method for making a GraphQL request.
    ///
    /// Takes one argument, `variables`. Returns an optional response.
    pub fn post<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
        headers: &HashMap<String, String>,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let h = headers::build(headers)?;
        let body = Q::build_query(variables);
        tracing::trace!(request_headers = ?h);
        tracing::trace!("Request Body: {}", serde_json::to_string(&body)?);

        let response = self.client.post(&self.uri).headers(h).json(&body).send()?;

        Client::handle_response::<Q>(response)
    }

    /// To be used internally or by other implementations of a graphql client.
    ///
    /// This fn tries to parse the JSON response from a graphql server. It will
    /// error if the JSON can't be parsed or if there are any graphql errors
    /// in the JSON body (in body.errors). If there are no errors, but an empty
    /// body.data, it will also error, as this shouldn't be possible.
    ///
    /// If successful, it will return body.data, unwrapped
    pub fn handle_response<Q: graphql_client::GraphQLQuery>(
        response: Response,
    ) -> Result<Q::ResponseData, RoverClientError> {
        tracing::debug!(response_status = ?response.status(), response_headers = ?response.headers());
        let response_text = response.text()?;
        tracing::debug!("{}", &response_text);
        let response_body: graphql_client::Response<Q::ResponseData> =
            serde_json::from_str(&response_text)?;

        if let Some(errs) = response_body.errors {
            if !errs.is_empty() && errs[0].message.contains("406") {
                return Err(RoverClientError::MalformedKey);
            }

            return Err(RoverClientError::GraphQL {
                msg: errs
                    .into_iter()
                    .map(|err| err.message)
                    .collect::<Vec<String>>()
                    .join("\n"),
            });
        }

        if let Some(data) = response_body.data {
            Ok(data)
        } else {
            Err(RoverClientError::MalformedResponse {
                null_field: "data".to_string(),
            })
        }
    }
}
