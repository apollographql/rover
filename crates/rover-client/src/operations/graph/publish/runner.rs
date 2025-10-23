use graphql_client::*;

use crate::{
    blocking::StudioClient,
    operations::graph::publish::{
        types::{ChangeSummary, FieldChanges, TypeChanges},
        GraphPublishInput, GraphPublishResponse,
    },
    shared::GraphRef,
    RoverClientError,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_publish_mutation
pub(crate) struct GraphPublishMutation;

/// Returns a message from apollo studio about the status of the update, and
/// a sha256 hash of the schema to be used with `schema publish`
pub async fn run(
    input: GraphPublishInput,
    client: &StudioClient,
) -> Result<GraphPublishResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<GraphPublishMutation>(input.into()).await?;
    let publish_response = get_publish_response_from_data(data, graph_ref)?;
    build_response(publish_response)
}

fn get_publish_response_from_data(
    data: graph_publish_mutation::ResponseData,
    graph_ref: GraphRef,
) -> Result<graph_publish_mutation::GraphPublishMutationGraphUploadSchema, RoverClientError> {
    // then, from the response data, get .service?.upload_schema?
    let graph = data
        .graph
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    graph
        .upload_schema
        .ok_or(RoverClientError::MalformedResponse {
            null_field: "service.upload_schema".to_string(),
        })
}

fn build_response(
    publish_response: graph_publish_mutation::GraphPublishMutationGraphUploadSchema,
) -> Result<GraphPublishResponse, RoverClientError> {
    if !publish_response.success {
        let msg = format!(
            "Schema upload failed with error: {}",
            publish_response.message
        );
        return Err(RoverClientError::AdhocError { msg });
    }

    let hash = match &publish_response.publication {
        // we only want to print the first 6 chars of a hash
        Some(publication) => publication.schema.hash.clone()[..6].to_string(),
        None => {
            let msg = format!(
                "No data in response from schema publish. Failed with message: {}",
                publish_response.message
            );
            return Err(RoverClientError::AdhocError { msg });
        }
    };

    // If you publish the exact same schema as is currently published,
    // the response CODE is NO_CHANGES but under the result diff,
    // it gives you the diff for that hash (i.e., the first time it was published)
    // which very well may have changes. For this, we'll just look at the code
    // first and handle the response as if there was `None` for the diff
    let change_summary = if publish_response.code == "NO_CHANGES" {
        ChangeSummary::none()
    } else {
        let diff = publish_response
            .publication
            .ok_or_else(|| RoverClientError::MalformedResponse {
                null_field: "service.upload_schema.publication".to_string(),
            })?
            .diff_to_previous;

        if let Some(diff) = diff {
            diff.into()
        } else {
            ChangeSummary::none()
        }
    };

    Ok(GraphPublishResponse {
        api_schema_hash: hash,
        change_summary,
    })
}

type QueryChangeDiff =
    graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationDiffToPrevious;

impl From<QueryChangeDiff> for ChangeSummary {
    fn from(input: QueryChangeDiff) -> Self {
        Self {
            field_changes: input.change_summary.field.into(),
            type_changes: input.change_summary.type_.into(),
        }
    }
}

type QueryFieldChanges =
graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationDiffToPreviousChangeSummaryField;

impl From<QueryFieldChanges> for FieldChanges {
    fn from(input: QueryFieldChanges) -> Self {
        Self::with_diff(
            input.additions as u64,
            input.removals as u64,
            input.edits as u64,
        )
    }
}

type QueryTypeChanges =
graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationDiffToPreviousChangeSummaryType;

impl From<QueryTypeChanges> for TypeChanges {
    fn from(input: QueryTypeChanges) -> Self {
        Self::with_diff(
            input.additions as u64,
            input.removals as u64,
            input.edits as u64,
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn get_publish_response_from_data_gets_data() {
        let json_response = json!({
            "graph": {
                "uploadSchema": {
                    "code": "IT_WERK",
                    "message": "it really do be published",
                    "success": true,
                    "publication": {
                        "variant": { "name": "current" },
                        "schema": { "hash": "123456" }
                    }
                }
            }
        });
        let data: graph_publish_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_publish_response_from_data(data, mock_graph_ref());

        assert!(output.is_ok());
        assert_eq!(
            output.unwrap(),
            graph_publish_mutation::GraphPublishMutationGraphUploadSchema {
                code: "IT_WERK".to_string(),
                message: "it really do be published".to_string(),
                success: true,
                publication: Some(graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublication {
                    variant: graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationVariant {
                        name: "current".to_string()
                    },
                    schema: graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationSchema {
                        hash: "123456".to_string()
                    },
                    diff_to_previous: None,
                }),
            }
        );
    }

    #[test]
    fn get_publish_response_from_data_errs_with_no_service() {
        let json_response = json!({ "service": null });
        let data: graph_publish_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_publish_response_from_data(data, mock_graph_ref());

        assert!(output.is_err());
    }

    #[test]
    fn get_publish_response_from_data_errs_with_no_upload_response() {
        let json_response = json!({
            "graph": {
                "uploadSchema": null
            }
        });
        let data: graph_publish_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_publish_response_from_data(data, mock_graph_ref());

        assert!(output.is_err());
    }

    #[test]
    fn build_response_struct_from_success() {
        let json_response = json!({
            "code": "IT_WERK",
            "message": "it really do be published",
            "success": true,
            "publication": {
                "variant": { "name": "current" },
                "schema": { "hash": "123456" }
            }
        });
        let update_response: graph_publish_mutation::GraphPublishMutationGraphUploadSchema =
            serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert!(output.is_ok());
        assert_eq!(
            output.unwrap(),
            GraphPublishResponse {
                api_schema_hash: "123456".to_string(),
                change_summary: ChangeSummary::none(),
            }
        );
    }

    #[test]
    fn build_response_errs_when_unsuccessful() {
        let json_response = json!({
            "code": "BAD_JOB",
            "message": "it really do be like that sometime",
            "success": false,
            "tag": null
        });
        let update_response: graph_publish_mutation::GraphPublishMutationGraphUploadSchema =
            serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert!(output.is_err());
    }

    #[test]
    fn build_response_errs_when_no_tag() {
        let json_response = json!({
            "code": "BAD_JOB",
            "message": "it really do be like that sometime",
            "success": true,
            "tag": null
        });
        let update_response: graph_publish_mutation::GraphPublishMutationGraphUploadSchema =
            serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert!(output.is_err());
    }

    #[test]
    fn build_change_summary_works_with_changes() {
        let json_diff = json!({
            "changeSummary": {
                "type": {
                "additions": 4,
                "removals": 0,
                "edits": 2
                },
                "field": {
                "additions": 3,
                "removals": 1,
                "edits": 0
                }
            }
        });
        let diff_to_previous: QueryChangeDiff = serde_json::from_value(json_diff).unwrap();
        let output: ChangeSummary = diff_to_previous.into();
        assert_eq!(
            output.to_string(),
            "[Fields: +3 -1 △ 0, Types: +4 -0 △ 2]".to_string()
        )
    }

    #[test]
    fn build_change_summary_works_with_no_changes() {
        assert_eq!(
            ChangeSummary::none().to_string(),
            "[No Changes]".to_string()
        )
    }

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }
}
