use crate::{headers, RoverClientError};
use graphql_client::GraphQLQuery;
use reqwest::{
    blocking::{Client as ReqwestClient, Response},
    StatusCode,
};
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

        let response = self
            .client
            .post(&self.uri)
            .headers(h)
            .json(&body)
            .send()
            .map_err(|e| {
                if e.is_connect() {
                    RoverClientError::CouldNotConnect {
                        url: e.url().cloned(),
                        source: e,
                    }
                } else {
                    e.into()
                }
            })?
            .error_for_status()?;

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

        match response.status() {
            StatusCode::OK => {
                let response_body: graphql_client::Response<Q::ResponseData> = response.json()?;

                if let Some(errs) = response_body.errors {
                    if !errs.is_empty() && errs[0].message.contains("406") {
                        return Err(RoverClientError::MalformedKey);
                    }

                    return Err(RoverClientError::GraphQl {
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
            // This block specifically handles an error that is returned when
            // Introspection is set to false on a production ApolloServer.
            //
            // We first check for a 400 HTTP Status Code (Bad Request). We then
            // get the message sent by the server and display that to our users.
            StatusCode::BAD_REQUEST => {
                // It's not a given that an HTTP response is valid JSON,
                // so let's match for a successful parse. Return a standard 400
                // RoverClientError if we are unable to parse.
                match response.json::<graphql_client::Response<Q::ResponseData>>() {
                    Ok(body) => {
                        if let Some(errs) = body.errors {
                            return Err(RoverClientError::ClientError {
                                msg: errs[0].message.to_string(),
                            });
                        }
                        Err(RoverClientError::ClientError {
                            msg: StatusCode::BAD_REQUEST.to_string(),
                        })
                    }
                    Err(_) => Err(RoverClientError::ClientError {
                        msg: StatusCode::BAD_REQUEST.to_string(),
                    }),
                }
            }
            status => Err(RoverClientError::ClientError {
                msg: status.to_string(),
            }),
        }
    }
}
