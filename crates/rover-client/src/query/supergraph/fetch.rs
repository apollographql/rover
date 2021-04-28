use std::unimplemented;

use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

// I'm not sure where this should live long-term
/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/supergraph/fetch.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. fetch_supergraph_query
pub struct FetchSupergraphQuery;

/// The main function to be used from this module. This function fetches a
/// core schema from apollo studio
pub fn run(
    variables: fetch_supergraph_query::Variables,
    client: &StudioClient,
) -> Result<String, RoverClientError> {
    let graph = variables.graph_id.clone();
    let variant = variables.variant.clone();
    let response_data = client.post::<FetchSupergraphQuery>(variables)?;
    get_supergraph_sdl_from_response_data(response_data, graph, variant)
}

fn get_supergraph_sdl_from_response_data(
    response_data: fetch_supergraph_query::ResponseData,
    graph: String,
    variant: String,
) -> Result<String, RoverClientError> {
    let service_data = match response_data.service {
        Some(data) => Ok(data),
        None => Err(RoverClientError::NoService {
            graph: graph.clone(),
        }),
    }?;

    if let Some(schema_tag) = service_data.schema_tag {
        if let Some(composition_result) = schema_tag.composition_result {
            if let Some(supergraph_sdl) = composition_result.supergraph_sdl {
                Ok(supergraph_sdl)
            } else {
                // TODO: why would we have composition_result but not supergraph_sdl
                // is this a server error?
                unimplemented!()
            }
        } else {
            // TODO: why would we have schema_tag but not composition_result
            // is this a server error?
            unimplemented!()
        }
    } else {
        // we're not quite sure _why_ schema tag is null, it could either be because
        // there is an invalid variant, or because there have been no successful
        // composition publishes for this supergraph.
        let mut valid_variants = Vec::new();

        for variant in service_data.variants {
            valid_variants.push(variant.name)
        }

        if !valid_variants.contains(&variant) {
            Err(RoverClientError::NoSchemaForVariant {
                graph,
                invalid_variant: variant,
                valid_variants,
                frontend_url_root: response_data.frontend_url_root,
            })
        } else {
            Err(RoverClientError::NoCompositionPublishes { graph })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn get_supergraph_sdl_from_response_data_works() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "service": {
                "schemaTag": {
                    "compositionResult": {
                        "supergraphSdl": "type Query { hello: String }",
                    },
                },
                "variants": []
            }
        });
        let data: fetch_supergraph_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let (graph, invalid_variant) = mock_vars();
        let output = get_supergraph_sdl_from_response_data(data, graph, invalid_variant);

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "type Query { hello: String }".to_string());
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_service() {
        let json_response =
            json!({ "service": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: fetch_supergraph_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let (graph, invalid_variant) = mock_vars();
        let output = get_supergraph_sdl_from_response_data(data, graph, invalid_variant);

        assert!(output.is_err());
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_result() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com/",
            "service": {
                "compositionResult": null,
                "variants": [],
            },
        });
        let data: fetch_supergraph_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let (graph, invalid_variant) = mock_vars();
        let output = get_supergraph_sdl_from_response_data(data, graph, invalid_variant);

        assert!(output.is_err());
    }

    fn mock_vars() -> (String, String) {
        ("mygraph".to_string(), "current".to_string())
    }
}
