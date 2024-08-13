use graphql_client::*;

use crate::blocking::StudioClient;
use crate::operations::cloud::config::types::CloudConfigUpdateInput;
use crate::shared::GraphRef;
use crate::RoverClientError;

use cloud_config_update_query::CloudConfigUpdateQueryGraphVariantUpsertRouterConfig::{
    GraphVariant, RouterUpsertFailure,
};
use cloud_config_update_query::CloudConfigUpdateQueryGraphVariantUpsertRouterConfigOnRouterUpsertFailure as OnRouterUpsertFailure;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/cloud/config/update_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct CloudConfigUpdateQuery;

pub async fn run(
    input: CloudConfigUpdateInput,
    client: &StudioClient,
) -> Result<(), RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<CloudConfigUpdateQuery>(input.into()).await?;
    build_response(graph_ref, data)
}

fn build_response(
    graph_ref: GraphRef,
    data: cloud_config_update_query::ResponseData,
) -> Result<(), RoverClientError> {
    let variant = data
        .graph
        .ok_or_else(|| RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?
        .variant
        .ok_or_else(|| RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?;

    match variant.upsert_router_config {
        // Router config successfully update.
        Some(GraphVariant { .. }) => Ok(()),
        // Error upserting router config.
        Some(RouterUpsertFailure(OnRouterUpsertFailure { message })) => {
            Err(RoverClientError::InvalidRouterConfig { msg: message })
        }
        // Invalid response returned from the API.
        None => Err(RoverClientError::MalformedResponse {
            null_field: "upsert_router_config".to_string(),
        }),
    }
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
    fn upsert_router_config_success() {
        let json_response = json!({
            "graph": {
                "variant": {
                    "upsertRouterConfig": {
                        "__typename": "GraphVariant",
                        "id": "123456789",
                    }
                },
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        assert!(output.is_ok());
    }

    #[test]
    fn null_upsert_router_config_error() {
        let json_response = json!({
            "graph": {
                "variant": {
                    "upsertRouterConfig": null
                },
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(mock_graph_ref(), data);

        match output.err() {
            Some(RoverClientError::MalformedResponse { .. }) => {}
            _ => panic!("expected malformed response error"),
        }
    }
}
