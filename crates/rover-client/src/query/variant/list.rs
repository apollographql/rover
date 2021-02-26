use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/variant/list.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. list_variants_query
pub struct ListVariantsQuery;

/// Fetches list of variants for a given graph
pub fn run(
    variables: list_variants_query::Variables,
    client: &StudioClient,
) -> Result<Vec<String>, RoverClientError> {
    let graph = variables.graph_id.clone();
    let response_data = client.post::<ListVariantsQuery>(variables)?;
    get_variants_from_response_data(response_data, graph.clone())
}

fn get_variants_from_response_data(
    response_data: list_variants_query::ResponseData,
    graph: String,
) -> Result<Vec<String>, RoverClientError> {
    let service_data = match response_data.service {
        Some(data) => Ok(data),
        None => Err(RoverClientError::NoService {
            graph: graph.clone(),
        }),
    }?;

    let mut res = Vec::new();

    for variant in service_data.variants {
        res.push(variant.name);
    }

    Ok(res)
}

mod tests {
    #[test]
    fn get_variants_from_response_data_works() {
        let json_response = serde_json::json!({
          "service": {
            "variants": [
                {
                    "name": "current"
                },
                {
                    "name": "dev"
                }
            ]
          }
        });

        let data: super::list_variants_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let actual_result =
            super::get_variants_from_response_data(data, "my-graph".to_string()).unwrap();

        let expected_result = vec!["current".to_string(), "dev".to_string()];

        assert_eq!(actual_result, expected_result);
    }

    #[test]
    fn get_variants_from_response_data_errs_with_null_service() {
        let json_response = serde_json::json!({ "service": null });
        let data: super::list_variants_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let actual_result = super::get_variants_from_response_data(data, "mygraph".to_string());
        assert!(actual_result.is_err());
    }
}
