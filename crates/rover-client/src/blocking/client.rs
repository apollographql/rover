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
    ) -> Result<Option<Q::ResponseData>, RoverClientError> {
        let h = headers::build(headers)?;
        let body = Q::build_query(variables);

        let response = self.client.post(&self.uri).headers(h).json(&body).send()?;

        Client::handle_response::<Q>(response)
    }

    /// To be used internally or by other implementations of a graphql client.
    ///
    /// This fn tries to parse the JSON response from a graphql server. It will
    /// error if the JSON can't be parsed or if there are any graphql errors
    /// in the JSON body (in body.errors).
    ///
    /// If successful, it will return body.data
    pub fn handle_response<Q: graphql_client::GraphQLQuery>(
        response: reqwest::blocking::Response,
    ) -> Result<Option<Q::ResponseData>, RoverClientError> {
        let response_body: graphql_client::Response<Q::ResponseData> =
            response
                .json()
                .map_err(|_| RoverClientError::HandleResponse {
                    msg: String::from("failed to parse response JSON"),
                })?;

        match response_body.errors {
            Some(errs) => Err(RoverClientError::GraphQL {
                msg: errs
                    .into_iter()
                    .map(|err| err.message)
                    .collect::<Vec<String>>()
                    .join("\n"),
            }),
            None => Ok(response_body.data),
        }
    }
}
