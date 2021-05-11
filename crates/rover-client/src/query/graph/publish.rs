use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/graph/publish.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. stash_schema_query
pub struct PublishSchemaMutation;

#[derive(Debug, PartialEq)]
pub struct PublishResponse {
    pub schema_hash: String,
    pub change_summary: String,
}

/// Returns a message from apollo studio about the status of the update, and
/// a sha256 hash of the schema to be used with `schema publish`
pub fn run(
    variables: publish_schema_mutation::Variables,
    client: &StudioClient,
) -> Result<PublishResponse, RoverClientError> {
    let graph = variables.graph_id.clone();
    let data = client.post::<PublishSchemaMutation>(variables)?;
    let publish_response = get_publish_response_from_data(data, graph)?;
    build_response(publish_response)
}

fn get_publish_response_from_data(
    data: publish_schema_mutation::ResponseData,
    graph: String,
) -> Result<publish_schema_mutation::PublishSchemaMutationServiceUploadSchema, RoverClientError> {
    // then, from the response data, get .service?.upload_schema?
    let service_data = match data.service {
        Some(data) => data,
        None => return Err(RoverClientError::NoService { graph }),
    };

    if let Some(opt_data) = service_data.upload_schema {
        Ok(opt_data)
    } else {
        Err(RoverClientError::MalformedResponse {
            null_field: "service.upload_schema".to_string(),
        })
    }
}

fn build_response(
    publish_response: publish_schema_mutation::PublishSchemaMutationServiceUploadSchema,
) -> Result<PublishResponse, RoverClientError> {
    if !publish_response.success {
        let msg = format!(
            "Schema upload failed with error: {}",
            publish_response.message
        );
        return Err(RoverClientError::AdhocError { msg });
    }

    let hash = match &publish_response.tag {
        // we only want to print the first 6 chars of a hash
        Some(tag_data) => tag_data.schema.hash.clone()[..6].to_string(),
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
        build_change_summary(None)
    } else {
        build_change_summary(publish_response.tag.unwrap().diff_to_previous)
    };

    Ok(PublishResponse {
        schema_hash: hash,
        change_summary,
    })
}

type ChangeDiff =
    publish_schema_mutation::PublishSchemaMutationServiceUploadSchemaTagDiffToPrevious;

/// builds a string-representation of the diff between two schemas
/// e.g. ` [Fields: +2 -1 △0, Types: +4 -0 △7]` or `[No Changes]`
fn build_change_summary(diff: Option<ChangeDiff>) -> String {
    match diff {
        None => "[No Changes]".to_string(),
        Some(diff) => {
            let changes = diff.change_summary;
            let fields = format!(
                "Fields: +{} -{} △ {}",
                changes.field.additions, changes.field.removals, changes.field.edits
            );
            let types = format!(
                "Types: +{} -{} △ {}",
                changes.type_.additions, changes.type_.removals, changes.type_.edits
            );
            format!("[{}, {}]", fields, types)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_publish_response_from_data_gets_data() {
        let json_response = json!({
            "service": {
                "uploadSchema": {
                    "code": "IT_WERK",
                    "message": "it really do be published",
                    "success": true,
                    "tag": {
                        "variant": { "name": "current" },
                        "schema": { "hash": "123456" }
                    }
                }
            }
        });
        let data: publish_schema_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_publish_response_from_data(data, "mygraph".to_string());

        assert!(output.is_ok());
        assert_eq!(
            output.unwrap(),
            publish_schema_mutation::PublishSchemaMutationServiceUploadSchema {
                code: "IT_WERK".to_string(),
                message: "it really do be published".to_string(),
                success: true,
                tag: Some(
                    publish_schema_mutation::PublishSchemaMutationServiceUploadSchemaTag {
                        variant:
                            publish_schema_mutation::PublishSchemaMutationServiceUploadSchemaTagVariant {
                                name: "current".to_string()
                            },
                        schema:
                            publish_schema_mutation::PublishSchemaMutationServiceUploadSchemaTagSchema {
                                hash: "123456".to_string()
                            },
                        diff_to_previous: None,
                    }
                )
            }
        );
    }

    #[test]
    fn get_publish_response_from_data_errs_with_no_service() {
        let json_response = json!({ "service": null });
        let data: publish_schema_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_publish_response_from_data(data, "mygraph".to_string());

        assert!(output.is_err());
    }

    #[test]
    fn get_publish_response_from_data_errs_with_no_upload_response() {
        let json_response = json!({
            "service": {
                "uploadSchema": null
            }
        });
        let data: publish_schema_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_publish_response_from_data(data, "mygraph".to_string());

        assert!(output.is_err());
    }

    #[test]
    fn build_response_struct_from_success() {
        let json_response = json!({
            "code": "IT_WERK",
            "message": "it really do be published",
            "success": true,
            "tag": {
                "variant": { "name": "current" },
                "schema": { "hash": "123456" }
            }
        });
        let update_response: publish_schema_mutation::PublishSchemaMutationServiceUploadSchema =
            serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert!(output.is_ok());
        assert_eq!(
            output.unwrap(),
            PublishResponse {
                schema_hash: "123456".to_string(),
                change_summary: "[No Changes]".to_string(),
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
        let update_response: publish_schema_mutation::PublishSchemaMutationServiceUploadSchema =
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
        let update_response: publish_schema_mutation::PublishSchemaMutationServiceUploadSchema =
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
        let diff_to_previous: ChangeDiff = serde_json::from_value(json_diff).unwrap();
        let output = build_change_summary(Some(diff_to_previous));
        assert_eq!(output, "[Fields: +3 -1 △ 0, Types: +4 -0 △ 2]".to_string())
    }

    #[test]
    fn build_change_summary_works_with_no_changes() {
        assert_eq!(build_change_summary(None), "[No Changes]".to_string())
    }
}
