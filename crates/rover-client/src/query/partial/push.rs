// PushPartialSchemaMutation
use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/partial/push.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. push_partial_schema_mutation
pub struct PushPartialSchemaMutation;

#[derive(Debug, PartialEq)]
pub struct PushPartialSchemaResponse {
    pub schema_hash: Option<String>,
    pub did_update_gateway: bool,
    pub service_was_created: bool,
    pub composition_errors: Option<Vec<String>>,
}

pub fn run(
    variables: push_partial_schema_mutation::Variables,
    client: &StudioClient,
) -> Result<PushPartialSchemaResponse, RoverClientError> {
    let data = client.post::<PushPartialSchemaMutation>(variables)?;
    let push_response = get_push_response_from_data(data)?;
    Ok(build_response(push_response))
}

// alias this return type since it's disgusting
type UpdateResponse = push_partial_schema_mutation::PushPartialSchemaMutationServiceUpsertImplementingServiceAndTriggerComposition;

fn get_push_response_from_data(
    data: push_partial_schema_mutation::ResponseData,
) -> Result<UpdateResponse, RoverClientError> {
    let service_data = match data.service {
        Some(data) => data,
        None => return Err(RoverClientError::NoService),
    };

    Ok(service_data.upsert_implementing_service_and_trigger_composition)
}

fn build_response(push_response: UpdateResponse) -> PushPartialSchemaResponse {
    let composition_errors: Vec<String> = push_response
        .errors
        .iter()
        .filter_map(|error| match error {
            Some(e) => Some(e.message.clone()),
            None => None,
        })
        .collect();

    // if there are no errors, just return None
    let composition_errors = if !composition_errors.is_empty() {
        Some(composition_errors)
    } else {
        None
    };

    PushPartialSchemaResponse {
        schema_hash: match push_response.composition_config {
            Some(config) => Some(config.schema_hash),
            None => None,
        },
        did_update_gateway: push_response.did_update_gateway,
        service_was_created: push_response.service_was_created,
        composition_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn build_response_works_with_composition_errors() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [
                {"message": "[Accounts] User -> composition error"},
                null, // this is technically allowed in the types
                {"message": "[Products] Product -> another one"}
            ],
            "didUpdateGateway": false,
            "serviceWasCreated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            PushPartialSchemaResponse {
                schema_hash: Some("5gf564".to_string()),
                composition_errors: Some(vec![
                    "[Accounts] User -> composition error".to_string(),
                    "[Products] Product -> another one".to_string()
                ]),
                did_update_gateway: false,
                service_was_created: true,
            }
        );
    }

    #[test]
    fn build_response_works_with_successful_composition() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [],
            "didUpdateGateway": true,
            "serviceWasCreated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            PushPartialSchemaResponse {
                schema_hash: Some("5gf564".to_string()),
                composition_errors: None,
                did_update_gateway: true,
                service_was_created: true,
            }
        );
    }

    // I think this case can happen when there are failures on the initial push
    // before composing? No service hash to return, and serviceWasCreated: false
    #[test]
    fn build_response_works_with_failure_and_no_hash() {
        let json_response = json!({
            "compositionConfig": null,
            "errors": [{ "message": "[Accounts] -> Things went really wrong" }],
            "didUpdateGateway": false,
            "serviceWasCreated": false
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            PushPartialSchemaResponse {
                schema_hash: None,
                composition_errors: Some(
                    vec!["[Accounts] -> Things went really wrong".to_string()]
                ),
                did_update_gateway: false,
                service_was_created: false,
            }
        );
    }
}
