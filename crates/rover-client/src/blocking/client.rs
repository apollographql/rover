use crate::RoverClientError;

use backoff::ExponentialBackoff;
use graphql_client::{Error as GraphQLError, GraphQLQuery, Response as GraphQLResponse};
use reqwest::{
    blocking::{Client as ReqwestClient, Response},
    header::{HeaderMap, HeaderName, HeaderValue},
    Error as ReqwestError, StatusCode,
};

use std::collections::HashMap;
use std::time::Duration;

pub(crate) const JSON_CONTENT_TYPE: &str = "application/json";
pub(crate) const CLIENT_NAME: &str = "rover-client";

/// Represents a generic GraphQL client for making http requests.
pub struct GraphQLClient {
    graphql_endpoint: String,
    client: ReqwestClient,
}

impl GraphQLClient {
    /// Construct a new [Client] from a `graphql_endpoint`.
    /// This client is used for generic GraphQL requests, such as introspection.
    pub fn new(
        graphql_endpoint: &str,
        client: ReqwestClient,
    ) -> Result<GraphQLClient, ReqwestError> {
        Ok(GraphQLClient {
            graphql_endpoint: graphql_endpoint.to_string(),
            client,
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
        let header_map = build_headers(header_map)?;
        let request_body = self.get_request_body::<Q>(variables)?;
        let response = self.execute(&request_body, header_map)?;
        GraphQLClient::handle_response::<Q>(response)
    }

    pub(crate) fn get_request_body<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<String, RoverClientError> {
        let body = Q::build_query(variables);
        Ok(serde_json::to_string(&body)?)
    }

    pub(crate) fn execute(
        &self,
        request_body: &str,
        header_map: HeaderMap,
    ) -> Result<Response, RoverClientError> {
        tracing::trace!(request_headers = ?header_map);
        tracing::debug!("Request Body: {}", request_body);
        let graphql_operation = || {
            let response = self
                .client
                .post(&self.graphql_endpoint)
                .headers(header_map.clone())
                .json(request_body)
                .send()
                .map_err(backoff::Error::Permanent)?;

            if let Err(status_error) = response.error_for_status_ref() {
                if let Some(response_status) = status_error.status() {
                    if response_status.is_server_error() {
                        Err(backoff::Error::Transient(status_error))
                    } else {
                        Err(backoff::Error::Permanent(status_error))
                    }
                } else {
                    Err(backoff::Error::Permanent(status_error))
                }
            } else {
                Ok(response)
            }
        };

        let backoff_strategy = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..Default::default()
        };

        backoff::retry(backoff_strategy, graphql_operation).map_err(|e| match e {
            backoff::Error::Permanent(reqwest_error) | backoff::Error::Transient(reqwest_error) => {
                if reqwest_error.is_connect() {
                    RoverClientError::CouldNotConnect {
                        url: reqwest_error.url().cloned(),
                        source: reqwest_error,
                    }
                } else {
                    reqwest_error.into()
                }
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

/// Function for building a [HeaderMap] for making http requests. Use for
/// Generic requests to any graphql endpoint.
///
/// Takes a single argument, list of header key/value pairs
fn build_headers(header_map: &HashMap<String, String>) -> Result<HeaderMap, RoverClientError> {
    let mut headers = HeaderMap::new();

    // this should be consistent for any graphql requests
    let content_type = HeaderValue::from_str(JSON_CONTENT_TYPE)?;
    headers.append("Content-Type", content_type);

    for (key, value) in header_map {
        let header_key = HeaderName::from_bytes(key.as_bytes())?;
        let header_value = HeaderValue::from_str(value)?;
        headers.append(header_key, header_value);
    }

    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

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

    #[test]
    fn test_successful_response() {
        let server = MockServer::start();
        let success_path = "/throw-me-a-frickin-bone-here";
        let success_mock = server.mock(|when, then| {
            when.method(POST).path(success_path);
            then.status(200).body("I'm the boss. I need the info.");
        });

        let client = ReqwestClient::new();
        let graphql_client = GraphQLClient::new(&server.url(success_path), client).unwrap();

        let response = graphql_client.execute("{}", HeaderMap::new());

        let mock_hits = success_mock.hits();

        if mock_hits != 1 {
            panic!("The request was never handled.");
        }

        assert!(response.is_ok())
    }

    #[test]
    fn test_unrecoverable_server_error() {
        let server = MockServer::start();
        let internal_server_error_path = "/this-is-me-in-a-nutshell";
        let internal_server_error_mock = server.mock(|when, then| {
            when.method(POST).path(internal_server_error_path);
            then.status(500).body("Help! I'm in a nutshell!");
        });

        let client = ReqwestClient::new();
        let graphql_client =
            GraphQLClient::new(&server.url(internal_server_error_path), client).unwrap();

        let response = graphql_client.execute("{}", HeaderMap::new());

        let mock_hits = internal_server_error_mock.hits();

        if mock_hits <= 1 {
            panic!("The request was never retried.");
        }

        assert!(response.is_err());
    }

    #[test]
    fn test_unrecoverable_client_error() {
        let server = MockServer::start();
        let not_found_path = "/austin-powers-the-musical";
        let not_found_mock = server.mock(|when, then| {
            when.method(POST).path(not_found_path);
            then.status(404).body("pretty sure that one never happened");
        });

        let client = ReqwestClient::new();
        let graphql_client = GraphQLClient::new(&server.url(not_found_path), client).unwrap();

        let response = graphql_client.execute("{}", HeaderMap::new());

        let mock_hits = not_found_mock.hits();

        if mock_hits != 1 {
            panic!("The request was never handled.");
        }

        let error = response.expect_err("Response didn't error");
        assert!(error.to_string().contains("Not Found"));
    }
}
