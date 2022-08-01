use crate::error::RoverClientError;

use graphql_client::{Error as GraphQLError, GraphQLQuery, Response as GraphQLResponse};
use reqwest::{
    blocking::{Client as ReqwestClient, Response},
    header::{HeaderMap, HeaderValue},
    Error as ReqwestError, StatusCode,
};

pub(crate) const JSON_CONTENT_TYPE: &str = "application/json";

const MAX_ELAPSED_TIME: Option<Duration> =
    Some(Duration::from_secs(if cfg!(test) { 2 } else { 10 }));

use std::time::Duration;

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
    /// Automatically retries requests.
    pub fn post<Q>(
        &self,
        variables: Q::Variables,
        header_map: &mut HeaderMap,
    ) -> Result<Q::ResponseData, RoverClientError>
    where
        Q: GraphQLQuery,
    {
        let request_body = self.get_request_body::<Q>(variables)?;
        header_map.append("Content-Type", HeaderValue::from_str(JSON_CONTENT_TYPE)?);
        let response = self.execute(request_body, header_map, true);
        GraphQLClient::handle_response::<Q>(response?)
    }

    /// Client method for making a GraphQL request.
    ///
    /// Takes one argument, `variables`. Returns an optional response.
    /// Does not automatically retry requests.
    pub fn post_no_retry<Q>(
        &self,
        variables: Q::Variables,
        header_map: &mut HeaderMap,
    ) -> Result<Q::ResponseData, RoverClientError>
    where
        Q: GraphQLQuery,
    {
        let request_body = self.get_request_body::<Q>(variables)?;
        header_map.append("Content-Type", HeaderValue::from_str(JSON_CONTENT_TYPE)?);
        let response = self.execute(request_body, header_map, false);
        GraphQLClient::handle_response::<Q>(response?)
    }

    fn get_request_body<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<String, RoverClientError> {
        let body = Q::build_query(variables);
        Ok(serde_json::to_string(&body)?)
    }

    fn execute(
        &self,
        request_body: String,
        header_map: &HeaderMap,
        should_retry: bool,
    ) -> Result<Response, RoverClientError> {
        use backoff::{retry, Error as BackoffError, ExponentialBackoff};

        tracing::trace!(request_headers = ?header_map);
        tracing::debug!("Request Body: {}", request_body);
        let graphql_operation = || {
            let response = self
                .client
                .post(&self.graphql_endpoint)
                .headers(header_map.clone())
                .body(request_body.clone())
                .send();

            match response {
                Err(client_error) => {
                    if client_error.is_timeout() || client_error.is_connect() {
                        Err(BackoffError::transient(client_error))
                    } else if client_error.is_body()
                        || client_error.is_decode()
                        || client_error.is_builder()
                        || client_error.is_redirect()
                    {
                        Err(BackoffError::Permanent(client_error))
                    } else if client_error.is_request() {
                        if let Some(hyper_error) =
                            get_source_error_type::<hyper::Error>(&client_error)
                        {
                            if hyper_error.is_incomplete_message() {
                                Err(BackoffError::transient(client_error))
                            } else {
                                Err(BackoffError::Permanent(client_error))
                            }
                        } else {
                            Err(BackoffError::Permanent(client_error))
                        }
                    } else {
                        Err(BackoffError::Permanent(client_error))
                    }
                }
                Ok(success) => {
                    if let Err(status_error) = success.error_for_status_ref() {
                        if let Some(response_status) = status_error.status() {
                            if response_status.is_server_error()
                                || response_status.is_client_error()
                                || response_status.is_redirection()
                            {
                                if matches!(response_status, StatusCode::BAD_REQUEST) {
                                    if let Ok(text) = success.text() {
                                        tracing::debug!("{}", text);
                                    }
                                    Err(BackoffError::Permanent(status_error))
                                } else {
                                    Err(BackoffError::transient(status_error))
                                }
                            } else {
                                Err(BackoffError::Permanent(status_error))
                            }
                        } else {
                            Err(BackoffError::Permanent(status_error))
                        }
                    } else {
                        Ok(success)
                    }
                }
            }
        };

        if should_retry {
            let backoff_strategy = ExponentialBackoff {
                max_elapsed_time: MAX_ELAPSED_TIME,
                ..Default::default()
            };

            retry(backoff_strategy, graphql_operation).map_err(|e| match e {
                BackoffError::Permanent(reqwest_error)
                | BackoffError::Transient {
                    err: reqwest_error,
                    retry_after: _,
                } => reqwest_error.into(),
            })
        } else {
            graphql_operation().map_err(|e| match e {
                BackoffError::Permanent(reqwest_error)
                | BackoffError::Transient {
                    err: reqwest_error,
                    retry_after: _,
                } => reqwest_error.into(),
            })
        }
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

/// Downcasts the given err source into T.
fn get_source_error_type<T: std::error::Error + 'static>(
    err: &dyn std::error::Error,
) -> Option<&T> {
    let mut source = err.source();

    while let Some(err) = source {
        if let Some(hyper_err) = err.downcast_ref::<T>() {
            return Some(hyper_err);
        }

        source = err.source();
    }
    None
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

        let response = graphql_client.execute("{}".to_string(), &HeaderMap::new(), true);

        let mock_hits = success_mock.hits();

        assert_eq!(mock_hits, 1);
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

        let response = graphql_client.execute("{}".to_string(), &HeaderMap::new(), true);

        let mock_hits = internal_server_error_mock.hits();

        assert!(mock_hits > 1);
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

        let response = graphql_client.execute("{}".to_string(), &HeaderMap::new(), true);

        let mock_hits = not_found_mock.hits();

        assert!(mock_hits > 1);

        let error = response.expect_err("Response didn't error");
        assert!(error.to_string().contains("Not Found"));
    }

    #[test]
    fn test_timeout_error() {
        let server = MockServer::start();
        let timeout_path = "/i-timeout-easily";
        let timeout_mock = server.mock(|when, then| {
            when.method(POST).path(timeout_path);
            then.status(200)
                .body("you've missed your bus")
                .delay(Duration::from_secs(3));
        });

        let client = reqwest::blocking::ClientBuilder::new()
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap();
        let graphql_client = GraphQLClient::new(&server.url(timeout_path), client).unwrap();

        let response = graphql_client.execute("{}".to_string(), &HeaderMap::new(), true);

        let mock_hits = timeout_mock.hits();

        assert!(mock_hits > 1);
        assert!(response.is_err());

        let error = response.expect_err("Response didn't error");
        assert!(error.to_string().contains("operation timed out"));
    }
}
