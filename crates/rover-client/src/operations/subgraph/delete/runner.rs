use apollo_federation_types::rover::{BuildError, BuildErrors};
use graphql_client::*;
use rover_studio::types::GraphRef;
use tower::{Service, ServiceExt};

use crate::{
    blocking::StudioClient,
    operations::subgraph::{
        delete::types::*,
        preview::{ComposeAndFilterPreviewInput, SubgraphChange},
    },
    shared::{AsyncBuildStatus, PreviewJobResponse},
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
/// `composeAndFilterPreviewAsync` build `rover subgraph preview` uses, using
/// an already-composed `Service`.
pub async fn check<S>(
    input: SubgraphDeleteInput,
    mut run_service: S,
) -> Result<SubgraphDeleteResponse, RoverClientError>
where
    S: Service<
        ComposeAndFilterPreviewInput,
        Response = PreviewJobResponse,
        Error = RoverClientError,
    >,
{
    let service = run_service.ready().await?;
    let preview_response = service
        .call(ComposeAndFilterPreviewInput {
            graph_ref: input.graph_ref,
            filter_config: None,
            subgraph_changes: vec![SubgraphChange {
                name: input.subgraph,
                info: None,
            }],
        })
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

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use super::{ComposeAndFilterPreviewInput, PreviewJobResponse, RoverClientError};

    rover_tower::mock_service!(
        ComposeAndFilterPreviewRun,
        ComposeAndFilterPreviewInput,
        PreviewJobResponse,
        RoverClientError
    );
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_tower::test::expect_poll_ready;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use speculoos::prelude::*;

    use super::{mock::MockComposeAndFilterPreviewRunService, *};
    use crate::shared::AsyncBuildStatus;

    #[fixture]
    fn test_input() -> SubgraphDeleteInput {
        SubgraphDeleteInput {
            graph_ref: "test-graph@test-variant".parse().unwrap(),
            subgraph: "accounts".to_string(),
        }
    }

    fn preview_response(status: AsyncBuildStatus, errors: Vec<String>) -> PreviewJobResponse {
        PreviewJobResponse {
            graph_ref: "test-graph@test-variant".parse().unwrap(),
            build_id: "build-123".to_string(),
            status,
            api_schema: None,
            supergraph_schema: None,
            errors,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn check_reports_success_when_composition_succeeds(test_input: SubgraphDeleteInput) {
        let mut mock = MockComposeAndFilterPreviewRunService::new();
        expect_poll_ready!(mock);
        mock.expect_call().return_once(|_| {
            future::ready(Ok(preview_response(AsyncBuildStatus::Success, Vec::new())))
        });

        let response = check(test_input, mock).await;

        assert_that!(response)
            .is_ok()
            .is_equal_to(SubgraphDeleteResponse {
                supergraph_was_updated: true,
                build_errors: BuildErrors::new(),
            });
    }

    #[rstest]
    #[tokio::test]
    async fn check_reports_build_errors_when_composition_fails(test_input: SubgraphDeleteInput) {
        let mut mock = MockComposeAndFilterPreviewRunService::new();
        expect_poll_ready!(mock);
        mock.expect_call().return_once(|_| {
            future::ready(Ok(preview_response(
                AsyncBuildStatus::ComposeFailed,
                vec!["accounts is required by products".to_string()],
            )))
        });

        let response = check(test_input, mock).await;

        assert_that!(response)
            .is_ok()
            .is_equal_to(SubgraphDeleteResponse {
                supergraph_was_updated: false,
                build_errors: vec![BuildError::composition_error(
                    None,
                    Some("accounts is required by products".to_string()),
                    None,
                    None,
                )]
                .into(),
            });
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
