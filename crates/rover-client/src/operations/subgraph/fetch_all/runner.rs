use graphql_client::*;

use crate::blocking::StudioClient;
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
pub fn run(
    input: SubgraphFetchAllInput,
    client: &StudioClient,
) -> Result<Vec<Subgraph>, RoverClientError> {
    let variables = input.clone().into();
    let response_data = client.post::<SubgraphFetchAllQuery>(variables)?;
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
        Some(SubgraphFetchAllGraphVariant::GraphVariant(variant)) => variant.subgraphs.map_or_else(
            || {
                Err(RoverClientError::ExpectedFederatedGraph {
                    graph_ref: input.graph_ref,
                    can_operation_convert: true,
                })
            },
            |subgraphs| {
                Ok(subgraphs
                    .into_iter()
                    .map(|subgraph| {
                        Subgraph::builder()
                            .name(subgraph.name.clone())
                            .and_url(subgraph.url)
                            .sdl(subgraph.active_partial_schema.sdl)
                            .build()
                    })
                    .collect())
            },
        ),
        _ => Err(RoverClientError::InvalidGraphRef),
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use serde_json::json;

    use crate::shared::GraphRef;

    use super::*;

    #[rstest]
    fn get_services_from_response_data_works(#[from(mock_input)] input: SubgraphFetchAllInput) {
        let sdl = "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n"
            .to_string();
        let url = "http://my.subgraph.com".to_string();
        let json_response = json!({
            "variant": {
                "__typename": "GraphVariant",
                "subgraphs": [
                    {
                        "name": "accounts",
                        "url": &url,
                        "activePartialSchema": {
                            "sdl": &sdl
                        }
                    },
                ]
            }
        });
        let data: SubgraphFetchAllResponseData = serde_json::from_value(json_response).unwrap();
        let expected_subgraph = Subgraph::builder()
            .url(url)
            .sdl(sdl)
            .name("accounts".to_string())
            .build();
        let output = get_subgraphs_from_response_data(input, data);

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), vec![expected_subgraph]);
    }

    #[rstest]
    fn get_services_from_response_data_errs_with_no_variant(mock_input: SubgraphFetchAllInput) {
        let json_response = json!({ "variant": null });
        let data: SubgraphFetchAllResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_subgraphs_from_response_data(mock_input, data);
        assert!(output.is_err());
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
