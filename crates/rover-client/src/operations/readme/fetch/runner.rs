use super::types::ReadmeFetchResponse;
use crate::blocking::StudioClient;
use crate::operations::readme::fetch::ReadmeFetchInput;
use crate::shared::GraphRef;
use crate::RoverClientError;
use graphql_client::*;

type Timestamp = String;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/readme/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct ReadmeFetchQuery;

pub async fn run(
    input: ReadmeFetchInput,
    client: &StudioClient,
) -> Result<ReadmeFetchResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<ReadmeFetchQuery>(input.into()).await?;
    build_response(data, graph_ref)
}

fn build_response(
    data: readme_fetch_query::ResponseData,
    graph_ref: GraphRef,
) -> Result<ReadmeFetchResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let valid_variants = graph.variants.iter().map(|it| it.name.clone()).collect();

    let variant = graph.variant.ok_or(RoverClientError::NoSchemaForVariant {
        graph_ref: graph_ref.clone(),
        valid_variants,
        frontend_url_root: data.frontend_url_root,
    })?;

    let readme = variant.readme;
    Ok(ReadmeFetchResponse {
        graph_ref,
        content: readme.content,
        last_updated_time: readme.last_updated_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::GraphRef;
    use serde_json::json;

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }

    #[test]
    fn get_readme_from_response_data_works() {
        let last_updated_time = "2022-05-12T20:50:06.687276000Z";
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "variant": {
                    "readme": {
                        "content": "this is a readme",
                        "lastUpdatedTime": last_updated_time,
                    },
                },
                "variants": []
            }
        });
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(data, mock_graph_ref());

        let expected_response = ReadmeFetchResponse {
            last_updated_time: Some(last_updated_time.to_string()),
            content: "this is a readme".to_string(),
            graph_ref: mock_graph_ref(),
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_response);
    }

    #[test]
    fn get_readme_from_response_data_errs_with_no_variant() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
            "variant": null,
            "variants": []
        }});
        let data = serde_json::from_value(json_response).unwrap();
        let output = build_response(data, mock_graph_ref());
        assert!(output.is_err());
    }
}
