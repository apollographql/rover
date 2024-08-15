use super::types::{CloudConfigValidateInput, CloudConfigValidateResponse};

use graphql_client::*;

use crate::blocking::StudioClient;
use crate::operations::cloud::config::validate::cloud_config_validate_query::{
    CloudConfigValidateQueryVariant::GraphVariant,
    CloudConfigValidateQueryVariantOnGraphVariantValidateRouter::{
        CloudValidationSuccess, InternalServerError, InvalidInputErrors,
    },
};
use crate::shared::GraphRef;
use crate::RoverClientError;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/cloud/config/validate_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct CloudConfigValidateQuery;

pub async fn run(
    input: CloudConfigValidateInput,
    client: &StudioClient,
) -> Result<CloudConfigValidateResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client
        .post::<CloudConfigValidateQuery>(input.into())
        .await?;
    build_response(graph_ref, data)
}

fn build_response(
    graph_ref: GraphRef,
    data: cloud_config_validate_query::ResponseData,
) -> Result<CloudConfigValidateResponse, RoverClientError> {
    let graph_variant = match data.variant {
        Some(GraphVariant(gv)) => gv,
        _ => {
            return Err(RoverClientError::GraphNotFound {
                graph_ref: graph_ref.clone(),
            })
        }
    };

    match graph_variant.validate_router {
        CloudValidationSuccess(res) => Ok(CloudConfigValidateResponse { msg: res.message }),
        InvalidInputErrors(e) => Err(RoverClientError::InvalidRouterConfig { msg: e.message }),
        InternalServerError(e) => Err(RoverClientError::ClientError { msg: e.message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::GraphRef;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }

    #[test]
    fn test_build_response_success() {
        let json_response = json!({
            "variant": {
                "__typename": "GraphVariant",
                "validateRouter": {
                    "__typename": "CloudValidationSuccess",
                    "message": "No errors!"
                }
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        let expected = CloudConfigValidateResponse {
            msg: "No errors!".to_string(),
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected);
    }

    #[test]
    fn test_build_response_errs_with_no_variant() {
        let json_response = json!({
            "variant": null
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        match output.err() {
            Some(RoverClientError::GraphNotFound { .. }) => {}
            _ => panic!("expected graph not found error"),
        }
    }

    #[test]
    fn test_build_response_errs_invalid_input() {
        let json_response = json!({
            "variant": {
                "__typename": "GraphVariant",
                "validateRouter": {
                    "__typename": "InvalidInputErrors",
                    "errors": [],
                    "message": "Invalid config"
                }
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        match output.err() {
            Some(RoverClientError::InvalidRouterConfig { msg }) => {
                assert_eq!("Invalid config".to_string(), msg)
            }
            _ => panic!("expected invalid router config error"),
        }
    }

    #[test]
    fn test_build_response_errs_internal_server_error() {
        let json_response = json!({
            "variant": {
                "__typename": "GraphVariant",
                "validateRouter": {
                    "__typename": "InternalServerError",
                    "message": "Client error"
                }
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        match output.err() {
            Some(RoverClientError::ClientError { msg }) => {
                assert_eq!("Client error".to_string(), msg)
            }
            _ => panic!("expected client error"),
        }
    }
}
