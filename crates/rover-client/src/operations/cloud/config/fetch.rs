use graphql_client::*;

use super::types::{CloudConfigFetchInput, CloudConfigFetchResponse};
use crate::{blocking::StudioClient, shared::GraphRef, RoverClientError};

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/cloud/config/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct CloudConfigFetchQuery;

pub async fn run(
    input: CloudConfigFetchInput,
    client: &StudioClient,
) -> Result<CloudConfigFetchResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<CloudConfigFetchQuery>(input.into()).await?;
    build_response(graph_ref, data)
}

fn build_response(
    graph_ref: GraphRef,
    data: cloud_config_fetch_query::ResponseData,
) -> Result<CloudConfigFetchResponse, RoverClientError> {
    let variant = data
        .graph
        .ok_or_else(|| RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?
        .variant
        .ok_or_else(|| RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?;

    // Router config will be non-null for any cloud-router variant, and null for any non-cloud variant/graph.
    let config = variant
        .router_config
        .ok_or_else(|| RoverClientError::NonCloudGraphRef {
            graph_ref: graph_ref.clone(),
        })?;

    Ok(CloudConfigFetchResponse { graph_ref, config })
}

#[cfg(test)]
#[expect(clippy::panic)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::shared::GraphRef;

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }

    #[test]
    fn test_build_response_success() {
        let json_response = json!({
            "graph": {
                "variant": {
                    "routerConfig": "some_config"
                }
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        let expected = CloudConfigFetchResponse {
            graph_ref: mock_graph_ref(),
            config: "some_config".to_string(),
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected);
    }

    #[test]
    fn test_build_response_errs_with_no_graph() {
        let json_response = json!({
            "graph": null,
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        match output.err() {
            Some(RoverClientError::GraphNotFound { .. }) => {}
            _ => panic!("expected graph not found error"),
        }
    }

    #[test]
    fn test_build_response_errs_with_no_variant() {
        let json_response = json!({
            "graph": {
                "variant": null,
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        match output.err() {
            Some(RoverClientError::GraphNotFound { .. }) => {}
            _ => panic!("expected graph not found error"),
        }
    }

    #[test]
    fn test_build_response_errs_with_non_cloud_router() {
        let json_response = json!({
            "graph": {
                "variant": {
                    "routerConfig": null
                }
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        match output.err() {
            Some(RoverClientError::NonCloudGraphRef { .. }) => {}
            _ => panic!("expected non-cloud graph error"),
        }
    }
}
