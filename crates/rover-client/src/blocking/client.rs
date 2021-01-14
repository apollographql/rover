use crate::headers;
use graphql_client::GraphQLQuery;
use rover_error::RoverError;
use std::collections::HashMap;

/// Represents a generic GraphQL client for making http requests.
pub struct Client {
    client: reqwest::blocking::Client,
    uri: String,
}

impl Client {
    /// Construct a new [Client] from a `uri`.
    /// This client is used for generic GraphQL requests, such as introspection.
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
    ) -> Result<Q::ResponseData, RoverError> {
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
    ) -> Result<Q::ResponseData, RoverError> {
        let response_body: graphql_client::Response<Q::ResponseData> =
            response.json().map_err(|_| RoverError::HandleResponse {
                msg: String::from("failed to parse response JSON"),
            })?;

        if let Some(errs) = response_body.errors {
            return Err(RoverError::GraphQL {
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
            Err(RoverError::NoData)
        }
    }
}
