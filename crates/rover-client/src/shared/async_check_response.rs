use std::fmt::Debug;

use http::StatusCode;
use rover_graphql::GraphQLServiceError;
use serde::Serialize;
use serde_json::{json, Value};

use crate::{error::EndpointKind, RoverClientError};

/// CheckRequestSuccessResult is the return type of the
/// `graph` and `subgraph` async check operations

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct CheckRequestSuccessResult {
    pub target_url: String,
    pub workflow_id: String,
}

/// Translates an error from submitting a check through the studio GraphQL service
/// into a [`RoverClientError`], shared by the `graph` and `subgraph` async check
/// submissions.
pub(crate) fn map_check_submission_error<T>(err: GraphQLServiceError<T>) -> RoverClientError
where
    T: Debug + Send + Sync,
{
    match err {
        GraphQLServiceError::Deserialization { status_code, .. }
            if status_code == StatusCode::PAYLOAD_TOO_LARGE =>
        {
            RoverClientError::RequestTooLarge {
                endpoint_kind: EndpointKind::ApolloStudio,
            }
        }
        other => other.into(),
    }
}

impl CheckRequestSuccessResult {
    pub fn get_json(&self) -> Value {
        json!({
            "target_url": self.target_url,
            "workflow_id": self.workflow_id,
        })
    }
}
