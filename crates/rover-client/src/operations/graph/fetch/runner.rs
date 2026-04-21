use tower::{Service, ServiceExt};

use super::{
    service::{GraphFetch, GraphFetchRequest},
    types::GraphFetchInput,
};
use crate::{blocking::StudioClient, shared::FetchResponse, RoverClientError};

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
    use serde_json::json;

    use super::super::service::graph_fetch_query;
    use super::super::service::get_schema_from_response_data;
    use rover_studio::types::GraphRef;

    fn mock_graph_ref() -> GraphRef {
        GraphRef::new("mygraph", Some("current")).unwrap()
    }

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
        let output = get_schema_from_response_data(data, mock_graph_ref());
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "type Query { hello: String }".to_string());
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_service() {
        let json_response =
            json!({ "service": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_schema_from_response_data(data, mock_graph_ref());
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
        let output = get_schema_from_response_data(data, mock_graph_ref());
        assert!(output.is_err());
    }
}
