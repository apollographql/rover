use super::types::*;
use crate::blocking::StudioClient;
use crate::operations::readme::publish::ReadmePublishInput;
use crate::shared::GraphRef;
use crate::RoverClientError;
use graphql_client::*;

type Timestamp = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/readme/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct ReadmePublishMutation;

pub async fn run(
    input: ReadmePublishInput,
    client: &StudioClient,
) -> Result<ReadmePublishResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response = client.post::<ReadmePublishMutation>(input.into()).await?;
    build_response(response, graph_ref)
}

fn build_response(
    data: readme_publish_mutation::ResponseData,
    graph_ref: GraphRef,
) -> Result<ReadmePublishResponse, RoverClientError> {
    let readme = data
        .graph
        .ok_or(RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?
        .variant
        .ok_or(RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?
        .update_variant_readme
        .ok_or(RoverClientError::MalformedResponse {
            null_field: "update_variant_readme".to_string(),
        })?
        .readme;
    Ok(ReadmePublishResponse {
        graph_ref,
        new_content: readme.content,
        last_updated_time: readme.last_updated_time,
    })
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
    fn get_new_readme_from_response_data_works() {
        let content = "this is a readme";
        let last_updated_time = "2022-05-12T20:50:06.687276000Z";

        let json_response = json!({
            "graph": {
                "variant": {
                    "updateVariantReadme": {
                        "readme": {
                            "content": content,
                            "lastUpdatedTime": last_updated_time,
                        }
                    }
                },
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = build_response(data, graph_ref.clone());

        let expected = ReadmePublishResponse {
            graph_ref,
            new_content: content.to_string(),
            last_updated_time: Some(last_updated_time.to_string()),
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected);
    }

    #[test]
    fn get_readme_errs_with_no_variant() {
        let json_response = json!({ "graph": { "variant": null  }});
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(data, mock_graph_ref());
        assert!(output.is_err());
    }

    #[test]
    fn null_update_variant_readme_errors() {
        let json_response = json!({
            "graph": {
                "variant": {
                    "updateVariantReadme": null
                },
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(data, mock_graph_ref());

        assert!(output.is_err());
    }
}
