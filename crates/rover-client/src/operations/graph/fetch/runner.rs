use crate::blocking::StudioClient;
use crate::operations::graph::fetch::GraphFetchInput;
use crate::shared::FetchResponse;
use crate::RoverClientError;

use graphql_client::*;

// I'm not sure where this should live long-term
/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_fetch_query
pub(crate) struct GraphFetchQuery;

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub fn run(
    input: GraphFetchInput,
    client: &StudioClient,
) -> Result<FetchResponse, RoverClientError> {
    let graph = input.graph_ref.name.clone();
    let invalid_variant = input.graph_ref.variant.clone();
    let response_data = client.post::<GraphFetchQuery>(input.into())?;
    let sdl = get_schema_from_response_data(response_data, graph, invalid_variant)?;
    Ok(FetchResponse { sdl })
}

fn get_schema_from_response_data(
    response_data: graph_fetch_query::ResponseData,
    graph: String,
    invalid_variant: String,
) -> Result<String, RoverClientError> {
    let service_data = response_data.service.ok_or(RoverClientError::NoService {
        graph: graph.clone(),
    })?;

    let mut valid_variants = Vec::new();

    for variant in service_data.variants {
        valid_variants.push(variant.name)
    }

    if let Some(schema) = service_data.schema {
        Ok(schema.document)
    } else {
        Err(RoverClientError::NoSchemaForVariant {
            graph,
            invalid_variant,
            valid_variants,
            frontend_url_root: response_data.frontend_url_root,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn get_schema_from_response_data_works() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "service": {
                "schema": {
                    "document": "type Query { hello: String }"
                },
                "variants": []
            }
        });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let (graph, invalid_variant) = mock_vars();
        let output = get_schema_from_response_data(data, graph, invalid_variant);

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "type Query { hello: String }".to_string());
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_service() {
        let json_response =
            json!({ "service": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let (graph, invalid_variant) = mock_vars();
        let output = get_schema_from_response_data(data, graph, invalid_variant);

        assert!(output.is_err());
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_schema() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com/",
            "service": {
                "schema": null,
                "variants": [],
            },
        });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let (graph, invalid_variant) = mock_vars();
        let output = get_schema_from_response_data(data, graph, invalid_variant);

        assert!(output.is_err());
    }

    fn mock_vars() -> (String, String) {
        ("mygraph".to_string(), "current".to_string())
    }
}
