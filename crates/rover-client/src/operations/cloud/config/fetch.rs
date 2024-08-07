use crate::blocking::StudioClient;
use crate::shared::GraphRef;
use crate::RoverClientError;
use graphql_client::*;

use super::types::{CloudConfigFetchInput, CloudConfigFetchResponse};

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

pub fn run(
    input: CloudConfigFetchInput,
    client: &StudioClient,
) -> Result<CloudConfigFetchResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<CloudConfigFetchQuery>(input.into())?;
    build_response(graph_ref, data)
}

fn build_response(
    graph_ref: GraphRef,
    data: cloud_config_fetch_query::ResponseData,
) -> Result<CloudConfigFetchResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let variant = graph.variant.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    // TODO: Add a check here? A router config will never be empty?
    let config = variant.router_config.unwrap();

    Ok(CloudConfigFetchResponse { graph_ref, config })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::GraphRef;
    use serde_json::json;

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }

    #[test]
    fn get_cloud_config_from_response_data_success() {
        let json_response = json!({
            "graph": {
                "variant": {
                    "routerConfig": "some_config"
                }
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        let expected_response = CloudConfigFetchResponse {
            graph_ref: mock_graph_ref(),
            config: "some_config".to_string(),
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_response);
    }

    #[test]
    fn get_cloud_config_from_response_data_errs_with_no_variant() {
        let json_response = json!({
            "graph": {
                "variant": null,
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);
        assert!(output.is_err());
    }
}
