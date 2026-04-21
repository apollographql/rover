use tower::{Service, ServiceExt};

use super::{
    service::{GraphFetch, GraphFetchRequest},
    types::GraphFetchInput,
};
use crate::{blocking::StudioClient, shared::FetchResponse, RoverClientError};

/// Fetch the SDL for the graph variant described by `input` using the given `client`.
pub async fn run(
    input: GraphFetchInput,
    client: &StudioClient,
) -> Result<FetchResponse, RoverClientError> {
    let mut service = GraphFetch::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    service.call(GraphFetchRequest::new(input)).await
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use serde_json::json;

    use crate::operations::graph::fetch::service::{get_schema_from_response_data, graph_fetch_query};
    use rover_studio::types::GraphRef;

    #[fixture]
    fn graph_ref() -> GraphRef {
        GraphRef::new("mygraph", Some("current")).unwrap()
    }

    /// Verifies that a response containing a schema document returns the SDL string successfully.
    #[rstest]
    fn get_schema_from_response_data_works(graph_ref: GraphRef) {
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
        let output = get_schema_from_response_data(data, graph_ref);
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "type Query { hello: String }".to_string());
    }

    /// Verifies that a null graph in the response produces an error.
    #[rstest]
    fn get_schema_from_response_data_errs_on_no_service(graph_ref: GraphRef) {
        let json_response =
            json!({ "service": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_schema_from_response_data(data, graph_ref);
        assert!(output.is_err());
    }

    /// Verifies that a response with a null variant (no published schema) produces an error.
    #[rstest]
    fn get_schema_from_response_data_errs_on_no_schema(graph_ref: GraphRef) {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com/",
            "graph": {
                "schema": null,
                "variants": [],
            },
        });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_schema_from_response_data(data, graph_ref);
        assert!(output.is_err());
    }
}
