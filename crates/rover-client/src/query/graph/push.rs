use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/graph/push.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. stash_schema_query
pub struct PushSchemaMutation;

#[derive(Debug, PartialEq)]
pub struct PushResponse {
    pub schema_hash: String,
    pub change_summary: String,
}

/// Returns a message from apollo studio about the status of the update, and
/// a sha256 hash of the schema to be used with `schema publish`
pub fn run(
    variables: push_schema_mutation::Variables,
    client: &StudioClient,
) -> Result<PushResponse, RoverClientError> {
    let data = client.post::<PushSchemaMutation>(variables)?;
    let push_response = get_push_response_from_data(data)?;
    build_response(push_response)
}

fn get_push_response_from_data(
    data: push_schema_mutation::ResponseData,
) -> Result<push_schema_mutation::PushSchemaMutationServiceUploadSchema, RoverClientError> {
    // then, from the response data, get .service?.upload_schema?
    let service_data = match data.service {
        Some(data) => data,
        None => return Err(RoverClientError::NoService),
    };

    if let Some(opt_data) = service_data.upload_schema {
        Ok(opt_data)
    } else {
        Err(RoverClientError::HandleResponse {
            msg: "No response from mutation. Check your API key & graph name".to_string(),
        })
    }
}

fn build_response(
    push_response: push_schema_mutation::PushSchemaMutationServiceUploadSchema,
) -> Result<PushResponse, RoverClientError> {
    if !push_response.success {
        let msg = format!("Schema upload failed with error: {}", push_response.message);
        return Err(RoverClientError::HandleResponse { msg });
    }

    let hash = match &push_response.tag {
        // we only want to print the first 6 chars of a hash
        Some(tag_data) => tag_data.schema.hash.clone()[..6].to_string(),
        None => {
            let msg = format!("No schema tag info available ({})", push_response.message);
            return Err(RoverClientError::HandleResponse { msg });
        }
    };

    // If you push the exact same schema as is currently published,
    // the response CODE is NO_CHANGES but under the result diff,
    // it gives you the diff for that hash (i.e., the first time it was pushed)
    // which very well may have changes. For this, we'll just look at the code
    // first and handle the response as if there was `None` for the diff
    let change_summary = if push_response.code == "NO_CHANGES" {
        build_change_summary(None)
    } else {
        build_change_summary(push_response.tag.unwrap().diff_to_previous)
    };

    Ok(PushResponse {
        schema_hash: hash,
        change_summary,
    })
}

type ChangeDiff = push_schema_mutation::PushSchemaMutationServiceUploadSchemaTagDiffToPrevious;

/// builds a string-representation of the diff between two schemas
/// e.g. ` [Fields: +2 -1 △0, Types: +4 -0 △7]` or `[No Changes]`
fn build_change_summary(diff: Option<ChangeDiff>) -> String {
    match diff {
        None => "[No Changes]".to_string(),
        Some(diff) => {
            let changes = diff.change_summary;
            let fields = format!(
                "Fields: +{} -{} △{}",
                changes.field.additions, changes.field.removals, changes.field.edits
            );
            let types = format!(
                "Types: +{} -{} △{}",
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
    fn get_push_response_from_data_gets_data() {
        let json_response = json!({
            "service": {
                "uploadSchema": {
                    "code": "IT_WERK",
                    "message": "it really do be pushed",
                    "success": true,
                    "tag": {
                        "variant": { "name": "current" },
                        "schema": { "hash": "123456" }
                    }
                }
            }
        });
        let data: push_schema_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_push_response_from_data(data);

        assert!(output.is_ok());
        assert_eq!(
            output.unwrap(),
            push_schema_mutation::PushSchemaMutationServiceUploadSchema {
                code: "IT_WERK".to_string(),
                message: "it really do be pushed".to_string(),
                success: true,
                tag: Some(
                    push_schema_mutation::PushSchemaMutationServiceUploadSchemaTag {
                        variant:
                            push_schema_mutation::PushSchemaMutationServiceUploadSchemaTagVariant {
                                name: "current".to_string()
                            },
                        schema:
                            push_schema_mutation::PushSchemaMutationServiceUploadSchemaTagSchema {
                                hash: "123456".to_string()
                            },
                        diff_to_previous: None,
                    }
                )
            }
        );
    }

    #[test]
    fn get_push_response_from_data_errs_with_no_service() {
        let json_response = json!({ "service": null });
        let data: push_schema_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_push_response_from_data(data);

        assert!(output.is_err());
    }

    #[test]
    fn get_push_response_from_data_errs_with_no_upload_response() {
        let json_response = json!({
            "service": {
                "uploadSchema": null
            }
        });
        let data: push_schema_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_push_response_from_data(data);

        assert!(output.is_err());
    }

    #[test]
    fn build_response_struct_from_success() {
        let json_response = json!({
            "code": "IT_WERK",
            "message": "it really do be pushed",
            "success": true,
            "tag": {
                "variant": { "name": "current" },
                "schema": { "hash": "123456" }
            }
        });
        let update_response: push_schema_mutation::PushSchemaMutationServiceUploadSchema =
            serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert!(output.is_ok());
        assert_eq!(
            output.unwrap(),
            PushResponse {
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
        let update_response: push_schema_mutation::PushSchemaMutationServiceUploadSchema =
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
        let update_response: push_schema_mutation::PushSchemaMutationServiceUploadSchema =
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
        assert_eq!(output, "[Fields: +3 -1 △0, Types: +4 -0 △2]".to_string())
    }

    #[test]
    fn build_change_summary_works_with_no_changes() {
        assert_eq!(build_change_summary(None), "[No Changes]".to_string())
    }
}
