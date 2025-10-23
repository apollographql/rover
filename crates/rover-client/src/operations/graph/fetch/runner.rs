use graphql_client::*;

use crate::{
    blocking::StudioClient,
    operations::graph::fetch::GraphFetchInput,
    shared::{FetchResponse, GraphRef, Sdl, SdlType},
    RoverClientError,
};

// I'm not sure where this should live long-term
/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_fetch_query
pub(crate) struct GraphFetchQuery;

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub async fn run(
    input: GraphFetchInput,
    client: &StudioClient,
) -> Result<FetchResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<GraphFetchQuery>(input.into()).await?;
    let sdl_contents = get_schema_from_response_data(response_data, graph_ref)?;
    Ok(FetchResponse {
        sdl: Sdl {
            contents: sdl_contents,
            r#type: SdlType::Graph,
        },
    })
}

fn get_schema_from_response_data(
    response_data: graph_fetch_query::ResponseData,
    graph_ref: GraphRef,
) -> Result<String, RoverClientError> {
    let graph = response_data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let mut valid_variants = Vec::new();

    for variant in graph.variants {
        valid_variants.push(variant.name)
    }

    if let Some(publication) = graph.variant.and_then(|it| it.latest_publication) {
        Ok(publication.schema.document)
    } else {
        Err(RoverClientError::NoSchemaForVariant {
            graph_ref,
            valid_variants,
            frontend_url_root: response_data.frontend_url_root,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn get_schema_from_response_data_works() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "variant": {
                    "latestPublication": {
                       "schema": {
                            "document": "type Query { hello: String }"
                        }
                    }
                },
                "variants": []
            }
        });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_schema_from_response_data(data, graph_ref);

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "type Query { hello: String }".to_string());
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_service() {
        let json_response =
            json!({ "service": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_schema_from_response_data(data, graph_ref);

        assert!(output.is_err());
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_schema() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com/",
            "graph": {
                "schema": null,
                "variants": [],
            },
        });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_schema_from_response_data(data, graph_ref);

        assert!(output.is_err());
    }

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }
}
