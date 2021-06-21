use crate::{headers, RoverClientError};
use graphql_client::{Error as GraphQLError, GraphQLQuery, Response as GraphQLResponse};
use reqwest::{
    blocking::{Client as ReqwestClient, Response},
    header::HeaderMap,
    Error as ReqwestError, StatusCode,
};

use std::collections::HashMap;

/// Represents a generic GraphQL client for making http requests.
pub struct GraphQLClient {
    client: ReqwestClient,
    graphql_endpoint: String,
}

impl GraphQLClient {
    /// Construct a new [Client] from a `graphql_endpoint`.
    /// This client is used for generic GraphQL requests, such as introspection.
    pub fn new(graphql_endpoint: &str) -> Result<GraphQLClient, ReqwestError> {
        Ok(GraphQLClient {
            client: ReqwestClient::builder()
                .use_rustls_tls()
                .gzip(true)
                .build()?,
            graphql_endpoint: graphql_endpoint.to_string(),
        })
    }

    /// Client method for making a GraphQL request.
    ///
    /// Takes one argument, `variables`. Returns an optional response.
    pub fn post<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
        header_map: &HashMap<String, String>,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let header_map = headers::build(header_map)?;
        let response = self.execute::<Q>(variables, header_map)?;
        GraphQLClient::handle_response::<Q>(response)
    }

    pub(crate) fn execute<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
        header_map: HeaderMap,
    ) -> Result<Response, RoverClientError> {
        let body = Q::build_query(variables);
        tracing::trace!(request_headers = ?header_map);
        tracing::debug!("Request Body: {}", serde_json::to_string(&body)?);
        self.client
            .post(&self.graphql_endpoint)
            .headers(header_map)
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
            })
    }

    /// To be used internally or by other implementations of a GraphQL client.
    ///
    /// This fn tries to parse the JSON response from a GraphQL server. It will
    /// error if the JSON can't be parsed or if there are any graphql errors
    /// in the JSON body (in body.errors). If there are no errors, but an empty
    /// body.data, it will also error, as this shouldn't be possible.
    ///
    /// If successful, it will return body.data, unwrapped
    pub(crate) fn handle_response<Q: GraphQLQuery>(
        response: Response,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let response_status = response.status();
        tracing::debug!(response_status = ?response_status, response_headers = ?response.headers());
        match response.json::<GraphQLResponse<Q::ResponseData>>() {
            Ok(response_body) => {
                if let Some(response_body_errors) = response_body.errors {
                    handle_graphql_body_errors(response_body_errors)?;
                }
                match response_status {
                    StatusCode::OK => {
                        response_body
                            .data
                            .ok_or_else(|| RoverClientError::MalformedResponse {
                                null_field: "data".to_string(),
                            })
                    }
                    status_code => Err(RoverClientError::ClientError {
                        msg: status_code.to_string(),
                    }),
                }
            }
            Err(e) => {
                if response_status.is_success() {
                    Err(e.into())
                } else {
                    Err(RoverClientError::ClientError {
                        msg: response_status.to_string(),
                    })
                }
            }
        }
    }
}

fn handle_graphql_body_errors(errors: Vec<GraphQLError>) -> Result<(), RoverClientError> {
    if errors.is_empty() {
        Ok(())
    } else if errors[0].message.contains("406") {
        Err(RoverClientError::MalformedKey)
    } else {
        Err(RoverClientError::GraphQl {
            msg: errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<String>>()
                .join("\n"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_ok_on_empty_errors() {
        let errors = vec![];
        assert!(handle_graphql_body_errors(errors).is_ok());
    }

    #[test]
    fn it_returns_malformed_key() {
        let errors = vec![GraphQLError {
            message: "406: Not Acceptable".to_string(),
            locations: None,
            extensions: None,
            path: None,
        }];
        let expected_error = RoverClientError::MalformedKey.to_string();
        let actual_error = handle_graphql_body_errors(errors).unwrap_err().to_string();
        assert_eq!(actual_error, expected_error);
    }

    #[test]
    fn it_returns_random_graphql_error() {
        let errors = vec![
            GraphQLError {
                message: "Something went wrong".to_string(),
                locations: None,
                extensions: None,
                path: None,
            },
            GraphQLError {
                message: "Something else went wrong".to_string(),
                locations: None,
                extensions: None,
                path: None,
            },
        ];
        let expected_error = RoverClientError::GraphQl {
            msg: format!("{}\n{}", errors[0].message, errors[1].message),
        }
        .to_string();
        let actual_error = handle_graphql_body_errors(errors).unwrap_err().to_string();
        assert_eq!(actual_error, expected_error);
    }
}
