use std::time::Duration;

use futures_util::TryFutureExt;
use graphql_client::{Error as GraphQLError, GraphQLQuery, Response as GraphQLResponse};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client as ReqwestClient, Response, StatusCode,
};

use crate::error::{EndpointKind, RoverClientError};

pub(crate) const JSON_CONTENT_TYPE: &str = "application/json";

const MAX_ELAPSED_TIME: Option<Duration> =
    Some(Duration::from_secs(if cfg!(test) { 2 } else { 10 }));

/// Represents a generic GraphQL client for making http requests.
pub struct GraphQLClient {
    graphql_endpoint: String,
    client: ReqwestClient,
}

impl GraphQLClient {
    /// Construct a new [Client] from a `graphql_endpoint`.
    /// This client is used for generic GraphQL requests, such as introspection.
    pub fn new(graphql_endpoint: &str, client: ReqwestClient) -> GraphQLClient {
        GraphQLClient {
            graphql_endpoint: graphql_endpoint.to_string(),
            client,
        }
    }

    /// Client method for making a GraphQL request.
    ///
    /// Takes one argument, `variables`. Returns an optional response.
    /// Automatically retries requests.
    pub async fn post<Q>(
        &self,
        variables: Q::Variables,
        header_map: &mut HeaderMap,
        endpoint_kind: EndpointKind,
    ) -> Result<Q::ResponseData, RoverClientError>
    where
        Q: GraphQLQuery,
    {
        let request_body = self.get_request_body::<Q>(variables)?;
        header_map.append("Content-Type", HeaderValue::from_str(JSON_CONTENT_TYPE)?);
        let response = self
            .execute(request_body, header_map, true, endpoint_kind)
            .await?;
        GraphQLClient::handle_response::<Q>(response, endpoint_kind).await
    }

    /// Client method for making a GraphQL request.
    ///
    /// Takes one argument, `variables`. Returns an optional response.
    /// Does not automatically retry requests.
    pub async fn post_no_retry<Q>(
        &self,
        variables: Q::Variables,
        header_map: &mut HeaderMap,
        endpoint_kind: EndpointKind,
    ) -> Result<Q::ResponseData, RoverClientError>
    where
        Q: GraphQLQuery,
    {
        let request_body = self.get_request_body::<Q>(variables)?;
        header_map.append("Content-Type", HeaderValue::from_str(JSON_CONTENT_TYPE)?);
        let response = self
            .execute(request_body, header_map, false, endpoint_kind)
            .await?;
        GraphQLClient::handle_response::<Q>(response, endpoint_kind).await
    }

    fn get_request_body<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<String, RoverClientError> {
        let body = Q::build_query(variables);
        Ok(serde_json::to_string(&body)?)
    }

    async fn execute(
        &self,
        request_body: String,
        header_map: &HeaderMap,
        should_retry: bool,
        endpoint_kind: EndpointKind,
    ) -> Result<Response, RoverClientError> {
        use backoff::{future::retry, Error as BackoffError, ExponentialBackoff};

        tracing::trace!(request_headers = ?header_map);
        tracing::debug!("Request Body: {}", request_body);
        let graphql_operation = || async {
            let response = self
                .client
                .post(&self.graphql_endpoint)
                .headers(header_map.clone())
                .body(request_body.clone())
                .send()
                .await;

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
                                    if let Ok(text) = success.text().await {
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

            retry(backoff_strategy, graphql_operation)
                .map_err(|e| RoverClientError::SendRequest {
                    source: e,
                    endpoint_kind,
                })
                .await
        } else {
            graphql_operation().await.map_err(|e| match e {
                BackoffError::Permanent(reqwest_error)
                | BackoffError::Transient {
                    err: reqwest_error,
                    retry_after: _,
                } => RoverClientError::SendRequest {
                    source: reqwest_error,
                    endpoint_kind,
                },
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
    pub(crate) async fn handle_response<Q: GraphQLQuery>(
        response: Response,
        endpoint_kind: EndpointKind,
    ) -> Result<Q::ResponseData, RoverClientError> {
        let response_status = response.status();
        tracing::debug!(response_status = ?response_status, response_headers = ?response.headers());
        match response.json::<GraphQLResponse<Q::ResponseData>>().await {
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
                    Err(RoverClientError::SendRequest {
                        source: e,
                        endpoint_kind,
                    })
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
    } else {
        tracing::debug!("GraphQL response errors: {:?}", errors);
        if errors[0].message == "406: Not Acceptable" {
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
