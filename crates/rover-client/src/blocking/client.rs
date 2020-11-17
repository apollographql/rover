use crate::headers;
use crate::RoverClientError;
use graphql_client::GraphQLQuery;
use std::collections::HashMap;

/// Represents a generic GraphQL client for making http requests.
pub struct Client {
    client: reqwest::blocking::Client,
    uri: String,
}

impl Client {
    /// Construct a new [StudioClient] from 2 strings, an `api_key` and a `uri`.
    /// For use in Rover, the `uri` is usually going to be to Apollo Studio
    pub fn new(uri: &str) -> Client {
        Client {
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
        headers: &HashMap<String, String>,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let h = headers::build(headers)?;
        let body = Q::build_query(variables);

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
        response: reqwest::blocking::Response,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let response_body: graphql_client::Response<Q::ResponseData> =
            response
                .json()
                .map_err(|_| RoverClientError::HandleResponse {
                    msg: String::from("failed to parse response JSON"),
                })?;

        if let Some(errs) = response_body.errors {
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
            Err(RoverClientError::HandleResponse {
                msg: "Response body's data was empty. This is probably a GraphQL execution error from the server.".to_string()
            })
        }
    }
}
