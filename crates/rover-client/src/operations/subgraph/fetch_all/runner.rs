use graphql_client::*;

use crate::blocking::StudioClient;
use crate::shared::GraphRef;
use crate::RoverClientError;

use super::types::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/fetch_all/fetch_all_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_fetch_all_query
pub(crate) struct SubgraphFetchAllQuery;

/// For a given graph return all of its subgraphs as a list
pub async fn run(
    input: SubgraphFetchAllInput,
    client: &StudioClient,
) -> Result<Vec<Subgraph>, RoverClientError> {
    let variables = input.clone().into();
    let response_data = client.post::<SubgraphFetchAllQuery>(variables).await?;
    get_subgraphs_from_response_data(input, response_data)
}

fn get_subgraphs_from_response_data(
    input: SubgraphFetchAllInput,
    response_data: SubgraphFetchAllResponseData,
) -> Result<Vec<Subgraph>, RoverClientError> {
    match response_data.variant {
        None => Err(RoverClientError::GraphNotFound {
            graph_ref: input.graph_ref,
        }),
        Some(SubgraphFetchAllGraphVariant::GraphVariant(variant)) => {
            extract_subgraphs_from_response(variant, input.graph_ref)
        }
        _ => Err(RoverClientError::InvalidGraphRef),
    }
}
fn extract_subgraphs_from_response(
    value: SubgraphFetchAllQueryVariantOnGraphVariant,
    graph_ref: GraphRef,
) -> Result<Vec<Subgraph>, RoverClientError> {
    match (value.subgraphs, value.source_variant) {
        // If we get null back in both branches or the query, or we get a structure in the
        // sourceVariant half but there are no subgraphs in it. Then we return an error
        // because this isn't a FederatedSubgraph **as far as we can tell**.
        (None, None)
        | (
            None,
            Some(SubgraphFetchAllQueryVariantOnGraphVariantSourceVariant { subgraphs: None }),
        ) => Err(RoverClientError::ExpectedFederatedGraph {
            graph_ref,
            can_operation_convert: true,
        }),
        // If we get nothing from querying the subgraphs directly, but we do get some subgraphs
        // on the sourceVariant side of the query, we just return those.
        (
            None,
            Some(SubgraphFetchAllQueryVariantOnGraphVariantSourceVariant {
                subgraphs: Some(subgraphs),
            }),
        ) => Ok(subgraphs
            .into_iter()
            .map(|subgraph| subgraph.into())
            .collect()),
        // Here there are three cases where we might want to return the subgraphs we got from
        // directly querying the graphVariant:
        // 1. If we get subgraphs back from the graphVariant directly and nothing from the sourceVariant
        // 2. If we get subgraphs back from the graphVariant directly and a structure from the
        // sourceVariant, but it contains no subgraphs
        // 3. If we get subgraphs back from both 'sides' of the query, we take the results from
        // querying the **graphVariant**, as this is closest to the original behaviour, before
        // we introduced the querying of the sourceVariant.
        (Some(subgraphs), _) => Ok(subgraphs
            .into_iter()
            .map(|subgraph| subgraph.into())
            .collect()),
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use serde_json::{json, Value};

    use crate::shared::GraphRef;

    use super::*;

    const SDL: &'static str =
        "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n";
    const URL: &'static str = "http://my.subgraph.com";
    const SUBGRAPH_NAME: &'static str = "accounts";

    #[rstest]
    #[case::subgraphs_returned_direct_from_variant(json!(
    {
        "variant": {
            "__typename": "GraphVariant",
            "subgraphs": [
                 {
                    "name": SUBGRAPH_NAME,
                    "url": URL,
                    "activePartialSchema": {
                        "sdl": SDL
                     }
                },
            ],
            "sourceVariant": null
        }
    }
    ), Some(vec![Subgraph::builder().url(URL).sdl(SDL).name(SUBGRAPH_NAME).build()]))]
    #[case::subgraphs_returned_via_source_variant(json!(
    {
        "variant": {
            "__typename": "GraphVariant",
            "subgraphs": null,
            "sourceVariant": {
                "subgraphs": [
                {
                    "name": SUBGRAPH_NAME,
                    "url": URL,
                    "activePartialSchema": {
                        "sdl": SDL
                    }
                }
                ]
            }
        }
    }), Some(vec![Subgraph::builder().url(URL).sdl(SDL).name(SUBGRAPH_NAME).build()]))]
    #[case::no_subgraphs_returned_in_either_case(json!(
    {
        "variant": {
            "__typename": "GraphVariant",
            "subgraphs": null,
            "sourceVariant": {
                "subgraphs": null
            }
        }
    }), None)]
    #[case::subgraphs_returned_from_both_sides_of_the_query_means_we_get_the_variants_subgraphs(json!(
    {
        "variant": {
        "__typename": "GraphVariant",
        "subgraphs": [
            {
                "name": SUBGRAPH_NAME,
                "url": URL,
                "activePartialSchema": {
                    "sdl": SDL
                }
            }
        ],
        "sourceVariant": {
            "subgraphs": [
                {
                    "name": "banana",
                    "url": URL,
                    "activePartialSchema": {
                        "sdl": SDL
                    }
                 }
             ]
        }
    }
    }), Some(vec![Subgraph::builder().url(URL).sdl(SDL).name(SUBGRAPH_NAME).build()]))]
    fn get_services_from_response_data_works(
        #[from(mock_input)] input: SubgraphFetchAllInput,
        #[case] json_response: Value,
        #[case] expected_subgraphs: Option<Vec<Subgraph>>,
    ) {
        let data: SubgraphFetchAllResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_subgraphs_from_response_data(input, data);

        if expected_subgraphs.is_some() {
            assert!(output.is_ok());
            assert_eq!(output.unwrap(), expected_subgraphs.unwrap());
        } else {
            assert!(output.is_err());
        };
    }

    #[fixture]
    fn mock_input() -> SubgraphFetchAllInput {
        let graph_ref = GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        };

        SubgraphFetchAllInput { graph_ref }
    }
}
