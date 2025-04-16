use super::types::*;
use crate::blocking::StudioClient;
use crate::RoverClientError;
use create_graph_mutation::CreateGraphMutationOrganizationCreateGraph;
use graphql_client::*;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/init/create_graph/create_graph_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. create_graph_mutation
pub(crate) struct CreateGraphMutation;

pub async fn run(
    input: CreateGraphInput,
    client: &StudioClient,
) -> Result<CreateGraphResponse, RoverClientError> {
    let variables: MutationVariables = input.clone().into();
    let data = client.post::<CreateGraphMutation>(variables).await?;
    let create_graph_response = build_response(data)?;
    Ok(create_graph_response)
}

fn build_response(data: ResponseData) -> Result<CreateGraphResponse, RoverClientError> {
    let graph_response = data
        .organization
        .ok_or_else(|| RoverClientError::MalformedResponse {
            null_field: "organization".to_string(),
        })?
        .create_graph;
    match graph_response {
        CreateGraphMutationOrganizationCreateGraph::Graph(graph) => Ok(CreateGraphResponse::from(
            CreateGraphMutationOrganizationCreateGraph::Graph(graph),
        )),
        CreateGraphMutationOrganizationCreateGraph::GraphCreationError(error) => {
            Err(RoverClientError::GraphCreationError { msg: error.message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_response_success() {
        let json_response = json!({
            "organization": {
                "createGraph": {
                    "__typename": "Graph",
                    "id": "123"
                }
            }
        });

        let data: create_graph_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let create_graph_response = build_response(data).unwrap();
        assert_eq!(create_graph_response.id, "123");
    }

    #[test]
    fn test_build_response_error() {
        let json_response = json!({
            "organization": {
                "createGraph": {
                    "__typename": "GraphCreationError",
                    "message": "Graph creation failed"
                }
            }
        });

        let data: create_graph_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let create_graph_response = build_response(data);
        assert!(create_graph_response.is_err());
    }
}
