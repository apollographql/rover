use apollo_federation_types::rover::{BuildError, BuildErrors};
use graphql_client::*;
use rover_studio::types::GraphRef;

use crate::{
    blocking::StudioClient,
    operations::subgraph::{
        delete::types::*,
        preview::{self, ComposeAndFilterPreviewInput, SubgraphChange},
    },
    shared::AsyncBuildStatus,
    RoverClientError,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/delete/delete_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_delete_mutation
pub(crate) struct SubgraphDeleteMutation;

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub async fn run(
    input: SubgraphDeleteInput,
    client: &StudioClient,
) -> Result<SubgraphDeleteResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<SubgraphDeleteMutation>(input.into()).await?;
    let data = get_delete_data_from_response(response_data, graph_ref)?;
    Ok(build_response(data))
}

/// Preview the composition impact of deleting a subgraph, via the same async
/// `composeAndFilterPreviewAsync` build `rover subgraph preview` uses.
pub async fn check(
    input: SubgraphDeleteInput,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<SubgraphDeleteResponse, RoverClientError> {
    let preview_response = preview::run(
        ComposeAndFilterPreviewInput {
            graph_ref: input.graph_ref,
            filter_config: None,
            subgraph_changes: vec![SubgraphChange {
                name: input.subgraph,
                info: None,
            }],
        },
        client,
        checks_timeout_seconds,
    )
    .await?;

    Ok(SubgraphDeleteResponse {
        supergraph_was_updated: preview_response.status == AsyncBuildStatus::Success,
        build_errors: preview_response
            .errors
            .into_iter()
            .map(|message| BuildError::composition_error(None, Some(message), None, None))
            .collect(),
    })
}

fn get_delete_data_from_response(
    response_data: subgraph_delete_mutation::ResponseData,
    graph_ref: GraphRef,
) -> Result<MutationComposition, RoverClientError> {
    let graph = response_data
        .graph
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    Ok(graph.remove_implementing_service_and_trigger_composition)
}

fn build_response(response: MutationComposition) -> SubgraphDeleteResponse {
    let build_errors: BuildErrors = response
        .errors
        .iter()
        .filter_map(|error| {
            error.as_ref().map(|e| {
                BuildError::composition_error(Some(e.message.clone()), e.code.clone(), None, None)
            })
        })
        .collect();

    SubgraphDeleteResponse {
        supergraph_was_updated: response.updated_gateway,
        build_errors,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use houston::{Credential, CredentialOrigin};
    use httpmock::prelude::*;
    use reqwest::Client as ReqwestClient;
    use serde_json::json;

    use super::*;

    fn test_client(server_url: &str) -> StudioClient {
        StudioClient::new(
            Credential {
                api_key: "test".to_string(),
                origin: CredentialOrigin::EnvVar,
                expires_at: None,
            },
            server_url,
            "test-version",
            false,
            ReqwestClient::new(),
            Duration::from_secs(1),
        )
    }

    fn test_input() -> SubgraphDeleteInput {
        SubgraphDeleteInput {
            graph_ref: "test-graph@test-variant".parse().unwrap(),
            subgraph: "accounts".to_string(),
        }
    }

    #[tokio::test]
    async fn check_reports_success_when_composition_succeeds() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewAsyncMutation");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewAsync": { "buildID": "build-123" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewStatusQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": { "__typename": "ComposeAndFilterPreviewSuccess" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": {
                        "__typename": "ComposeAndFilterPreviewSuccess",
                        "composeResults": {
                            "apiSchemaDocument": "type Query { hi: String }",
                            "supergraphSchemaDocument": "supergraph"
                        },
                        "filterResults": null
                    }
                } } }
            }));
        });

        let response = check(test_input(), &test_client(&server.url("/")), 30)
            .await
            .unwrap();

        assert_eq!(
            response,
            SubgraphDeleteResponse {
                supergraph_was_updated: true,
                build_errors: BuildErrors::new(),
            }
        );
    }

    #[tokio::test]
    async fn check_reports_build_errors_when_composition_fails() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewAsyncMutation");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewAsync": { "buildID": "build-123" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewStatusQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": { "__typename": "ComposeAndFilterPreviewComposeFailure" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": {
                        "__typename": "ComposeAndFilterPreviewComposeFailure",
                        "composeErrors": [
                            { "message": "accounts is required by products", "code": "REQUIRED_SUBGRAPH", "failedStep": "VALIDATE" }
                        ]
                    }
                } } }
            }));
        });

        let response = check(test_input(), &test_client(&server.url("/")), 30)
            .await
            .unwrap();

        assert_eq!(
            response,
            SubgraphDeleteResponse {
                supergraph_was_updated: false,
                build_errors: vec![BuildError::composition_error(
                    None,
                    Some("accounts is required by products".to_string()),
                    None,
                    None
                )]
                .into(),
            }
        );
    }

    #[test]
    fn get_delete_data_from_response_works() {
        let json_response = json!({
            "graph": {
                "removeImplementingServiceAndTriggerComposition": {
                    "errors": [
                        {
                            "message": "wow",
                            "code": null
                        },
                        null,
                        {
                           "message": "boo",
                           "code": "BOO"
                        }
                    ],
                    "updatedGateway": false,
                }
            }
        });
        let data: subgraph_delete_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_delete_data_from_response(data, mock_graph_ref());

        assert!(output.is_ok());

        let expected_response = MutationComposition {
            errors: vec![
                Some(MutationCompositionErrors {
                    message: "wow".to_string(),
                    code: None,
                }),
                None,
                Some(MutationCompositionErrors {
                    message: "boo".to_string(),
                    code: Some("BOO".to_string()),
                }),
            ],
            updated_gateway: false,
        };
        assert_eq!(output.unwrap(), expected_response);
    }

    #[test]
    fn build_response_works_with_successful_responses() {
        let response = MutationComposition {
            errors: vec![
                Some(MutationCompositionErrors {
                    message: "wow".to_string(),
                    code: None,
                }),
                None,
                Some(MutationCompositionErrors {
                    message: "boo".to_string(),
                    code: Some("BOO".to_string()),
                }),
            ],
            updated_gateway: false,
        };

        let parsed = build_response(response);
        assert_eq!(
            parsed,
            SubgraphDeleteResponse {
                build_errors: vec![
                    BuildError::composition_error(Some("wow".to_string()), None, None, None),
                    BuildError::composition_error(
                        Some("boo".to_string()),
                        Some("BOO".to_string()),
                        None,
                        None
                    )
                ]
                .into(),
                supergraph_was_updated: false,
            }
        );
    }

    #[test]
    fn build_response_works_with_failure_responses() {
        let response = MutationComposition {
            errors: vec![],
            updated_gateway: true,
        };

        let parsed = build_response(response);
        assert_eq!(
            parsed,
            SubgraphDeleteResponse {
                build_errors: BuildErrors::new(),
                supergraph_was_updated: true,
            }
        );
    }

    fn mock_graph_ref() -> GraphRef {
        GraphRef::new("mygraph", Some("current")).unwrap()
    }
}
