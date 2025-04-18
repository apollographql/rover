use super::types::*;
use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/init/build_pipeline_track/build_pipeline_track_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. create_graph_mutation
pub(crate) struct BuildPipelineTrackMutation;

pub async fn run(
    input: BuildPipelineTrackInput,
    client: &StudioClient,
) -> Result<BuildPipelineTrackResponse, RoverClientError> {
    let variables: MutationVariables = input.into();
    let data = client.post::<BuildPipelineTrackMutation>(variables).await?;
    let build_pipeline_track_response = build_response(data)?;
    Ok(build_pipeline_track_response)
}

fn build_response(data: ResponseData) -> Result<BuildPipelineTrackResponse, RoverClientError> {
    let graph_response = data
        .graph
        .ok_or_else(|| RoverClientError::MalformedResponse {
            null_field: "graph".to_string(),
        })?
        .variant
        .ok_or_else(|| RoverClientError::MalformedResponse {
            null_field: "variant".to_string(),
        })?
        .update_variant_federation_version
        .ok_or_else(|| RoverClientError::MalformedResponse {
            null_field: "updateVariantFederationVersion".to_string(),
        })?;
    Ok(BuildPipelineTrackResponse::from(graph_response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_response_success() {
        let json_response = json!({
            "graph": {
                "variant": {
                    "updateVariantFederationVersion": {
                        "__typename": "Graph",
                        "id": "123"
                    }
                }
            }
        });

        let data: build_pipeline_track_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let build_pipeline_track_response = build_response(data).unwrap();
        assert_eq!(build_pipeline_track_response.id, "123");
    }

    #[test]
    fn test_build_response_error() {
        let json_response = json!({
            "graph": {
                "variant": {
                    "updateVariantFederationVersion": {
                        "__typename": "BuildPipelineTrackError",
                        "message": "Build pipeline track failed"
                    }
                }
            }
        });

        let data: build_pipeline_track_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let build_pipeline_track_response = build_response(data);
        assert!(build_pipeline_track_response.is_err());
    }
}
