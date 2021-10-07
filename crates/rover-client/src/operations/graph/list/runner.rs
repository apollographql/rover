use crate::blocking::StudioClient;
use crate::operations::graph::list::types::*;
use crate::operations::graph::list::GraphListInput;
use crate::shared::GraphRef;
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/list/list_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_list_query
pub(crate) struct GraphListQuery;

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub fn run(
    input: GraphListInput,
    client: &StudioClient,
) -> Result<GraphListResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<GraphListQuery>(input.into())?;
    let variants = get_variants_from_response_data(response_data, graph_ref)?;
    Ok(GraphListResponse {
        variants: format_variants(&variants),
    })
}

fn get_variants_from_response_data(
    response_data: graph_list_query::ResponseData,
    graph_ref: GraphRef,
) -> Result<Vec<GraphListQueryVariantInfo>, RoverClientError> {
    let service_data = response_data
        .service
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    Ok(service_data.variants)
}

fn format_variants(vars: &[GraphListQueryVariantInfo]) -> Vec<GraphVariant> {
    let variants: Vec<GraphVariant> = vars
        .iter()
        .map(|variant| GraphVariant {
            id: variant.id.clone(),
            name: variant.name.clone(),
            is_protected: variant.is_protected,
            is_public: variant.is_public,
        })
        .collect();

    variants
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn get_variants_from_response_data_works() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "service": {
                "variants": [{
                    "id": "mygraph@v1",
                    "name": "v1",
                    "isProtected": true,
                    "isPublic": false
                }, {
                    "id": "mygraph@v2",
                    "name": "v2",
                    "isProtected": false,
                    "isPublic": true
                }]
            }
        });
        let data: graph_list_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output: Result<Vec<GraphListQueryVariantInfo>, RoverClientError> =
            get_variants_from_response_data(data, graph_ref);

        let expected_json = json!([{
            "id": "mygraph@v1",
            "name": "v1",
            "isProtected": true,
            "isPublic": false
        }, {
            "id": "mygraph@v2",
            "name": "v2",
            "isProtected": false,
            "isPublic": true
        }]);
        let expected_variants_list: Vec<GraphListQueryVariantInfo> =
            serde_json::from_value(expected_json).unwrap();

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_variants_list);
    }

    #[test]
    fn get_variants_from_response_data_errs_on_no_service() {
        let json_response =
            json!({ "service": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: graph_list_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_variants_from_response_data(data, graph_ref);

        assert!(output.is_err());
    }

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }
}
