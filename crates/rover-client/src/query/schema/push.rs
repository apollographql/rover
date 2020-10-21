use crate::blocking::Client;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/schema/push.graphql",
    schema_path = "schema.graphql",
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
    pub message: String,
}

/// Returns a message from apollo studio about the status of the update, and
/// a sha256 hash of the schema to be used with `schema publish`
pub fn run(
    variables: push_schema_mutation::Variables,
    client: Client,
) -> Result<PushResponse, RoverClientError> {
    let data = execute_mutation(client, variables)?;
    let push_response = get_push_response_from_data(data)?;
    build_response(push_response)
}

fn execute_mutation(
    client: Client,
    variables: push_schema_mutation::Variables,
) -> Result<push_schema_mutation::ResponseData, RoverClientError> {
    let res = client.post::<PushSchemaMutation>(variables)?;
    if let Some(opt_res) = res {
        Ok(opt_res)
    } else {
        Err(RoverClientError::ResponseError {
            msg: "Error fetching schema. Check your API key & graph id".to_string(),
        })
    }
}

fn get_push_response_from_data(
    data: push_schema_mutation::ResponseData,
) -> Result<push_schema_mutation::PushSchemaMutationServiceUploadSchema, RoverClientError> {
    // then, from the response data, get .service?.upload_schema?
    let service_data = match data.service {
        Some(data) => data,
        None => {
            return Err(RoverClientError::ResponseError {
                msg: "No response from mutation. Check your API key & graph id".to_string(),
            })
        }
    };

    if let Some(opt_data) = service_data.upload_schema {
        Ok(opt_data)
    } else {
        Err(RoverClientError::ResponseError {
            msg: "No response from mutation. Check your API key & graph name".to_string(),
        })
    }
}

fn build_response(
    push_response: push_schema_mutation::PushSchemaMutationServiceUploadSchema,
) -> Result<PushResponse, RoverClientError> {
    if !push_response.success {
        let msg = format!("Schema upload failed with error: {}", push_response.message);
        return Err(RoverClientError::ResponseError { msg });
    }

    let hash = match push_response.tag {
        Some(tag_data) => tag_data.schema.hash,
        None => {
            let msg = format!("No schema tag info available ({})", push_response.message);
            return Err(RoverClientError::ResponseError { msg });
        }
    };

    Ok(PushResponse {
        message: push_response.message,
        schema_hash: hash,
    })
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
                            }
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
    fn build_resposne_struct_from_success() {
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
                message: "it really do be pushed".to_string()
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
}
