use std::{
    thread,
    time::{Duration, Instant},
};

use apollo_federation_types::rover::{BuildError, BuildErrors};
use graphql_client::*;
use rover_studio::types::GraphRef;

use self::subgraph_publish_launch_status_query::{
    LaunchStatus, SubgraphPublishLaunchStatusQueryGraphVariantLaunch,
};
use super::types::*;
use crate::{
    blocking::StudioClient,
    error::FailedLaunch,
    operations::{
        config::is_federated::{self, IsFederatedInput},
        graph::{variant, variant::VariantListInput},
    },
    RoverClientError,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_publish_mutation
pub(crate) struct SubgraphPublishMutation;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/publish/launch_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
pub(crate) struct SubgraphPublishLaunchStatusQuery;

pub async fn run(
    input: SubgraphPublishInput,
    client: &StudioClient,
) -> Result<SubgraphPublishResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let variables: MutationVariables = input.clone().into();
    // We don't want to implicitly convert non-federated graph to supergraphs.
    // Error here if no --convert flag is passed _and_ the current context
    // is non-federated. Add a suggestion to require a --convert flag.
    if !input.convert_to_federated_graph {
        // first, check if the variant exists _at all_
        // if it doesn't exist, there is no graph schema to "convert"
        // so don't require --convert in this case, just publish the subgraph
        let variant_exists = variant::run(
            VariantListInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )
        .await
        .is_ok();

        if variant_exists {
            // check if subgraphs have ever been published to this graph ref
            let is_federated = is_federated::run(
                IsFederatedInput {
                    graph_ref: graph_ref.clone(),
                },
                client,
            )
            .await?;

            if !is_federated {
                return Err(RoverClientError::ExpectedFederatedGraph {
                    graph_ref,
                    can_operation_convert: true,
                });
            }
        } else {
            tracing::debug!(
                "Publishing new subgraph {} to {}",
                &input.subgraph,
                &input.graph_ref
            );
        }
    }
    let data = client.post::<SubgraphPublishMutation>(variables).await?;
    let publish_response = get_publish_response_from_data(data, graph_ref.clone())?;
    if let Some(launch_id) = publish_response
        .launch
        .as_ref()
        .map(|launch| launch.id.clone())
    {
        let launch = poll_launch(
            &graph_ref,
            &launch_id,
            input.launch_poll_timeout_seconds,
            client,
        )
        .await?;
        ensure_launch_succeeded(&graph_ref, &launch)?;
    }
    Ok(build_response(publish_response))
}

fn get_publish_response_from_data(
    data: ResponseData,
    graph_ref: GraphRef,
) -> Result<UpdateResponse, RoverClientError> {
    let graph = data
        .graph
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    graph
        .publish_subgraph
        .ok_or(RoverClientError::MalformedResponse {
            null_field: "service.upsertImplementingServiceAndTriggerComposition".to_string(),
        })
}

async fn poll_launch(
    graph_ref: &GraphRef,
    launch_id: &str,
    timeout_seconds: u64,
    client: &StudioClient,
) -> Result<SubgraphPublishLaunchStatusQueryGraphVariantLaunch, RoverClientError> {
    let now = Instant::now();
    let launch_url = Some(launch_url(graph_ref.graph_id(), launch_id));

    loop {
        match fetch_launch(graph_ref, launch_id, client).await {
            Ok(launch) => {
                if launch_is_finished(&launch) {
                    return Ok(launch);
                }
            }
            Err(e) if e.is_transient() => {
                eprintln!("error while checking status of launch: {e}\nretrying...");
            }
            Err(e) => return Err(e),
        }

        if now.elapsed() > Duration::from_secs(timeout_seconds) {
            return Err(RoverClientError::LaunchTimeoutError { url: launch_url });
        }
        thread::sleep(Duration::from_secs(5));
    }
}

async fn fetch_launch(
    graph_ref: &GraphRef,
    launch_id: &str,
    client: &StudioClient,
) -> Result<SubgraphPublishLaunchStatusQueryGraphVariantLaunch, RoverClientError> {
    let (graph_id, variant) = graph_ref.clone().into_parts();
    let data = client
        .post::<SubgraphPublishLaunchStatusQuery>(LaunchStatusVariables {
            graph_id,
            variant,
            launch_id: launch_id.to_string(),
        })
        .await?;

    get_launch_from_data(data, graph_ref.clone())
}

fn get_launch_from_data(
    data: LaunchStatusResponseData,
    graph_ref: GraphRef,
) -> Result<SubgraphPublishLaunchStatusQueryGraphVariantLaunch, RoverClientError> {
    let graph = data
        .graph
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    let variant = graph.variant.ok_or(RoverClientError::MalformedResponse {
        null_field: "graph.variant".to_string(),
    })?;

    variant.launch.ok_or(RoverClientError::MalformedResponse {
        null_field: "graph.variant.launch".to_string(),
    })
}

fn launch_is_finished(launch: &SubgraphPublishLaunchStatusQueryGraphVariantLaunch) -> bool {
    !launch_status_is_pending(&launch.status)
        && launch
            .downstream_launches
            .iter()
            .all(|launch| !launch_status_is_pending(&launch.status))
}

fn ensure_launch_succeeded(
    graph_ref: &GraphRef,
    launch: &SubgraphPublishLaunchStatusQueryGraphVariantLaunch,
) -> Result<(), RoverClientError> {
    let failed_downstream_launches = launch
        .downstream_launches
        .iter()
        .filter(|launch| launch_status_is_failed(&launch.status))
        .map(|launch| FailedLaunch {
            graph_id: launch.graph_id.clone(),
            graph_variant: launch.graph_variant.clone(),
            launch_id: launch.id.clone(),
        })
        .collect::<Vec<_>>();

    if launch_status_is_failed(&launch.status) || !failed_downstream_launches.is_empty() {
        Err(RoverClientError::SubgraphPublishLaunchFailure {
            graph_ref: graph_ref.clone(),
            launch_id: launch.id.clone(),
            failed_downstream_launches,
        })
    } else {
        Ok(())
    }
}

fn launch_status_is_pending(status: &LaunchStatus) -> bool {
    matches!(status, LaunchStatus::LAUNCH_INITIATED)
}

fn launch_status_is_failed(status: &LaunchStatus) -> bool {
    !matches!(
        status,
        LaunchStatus::LAUNCH_COMPLETED | LaunchStatus::LAUNCH_INITIATED
    )
}

fn launch_url(graph_id: &str, launch_id: &str) -> String {
    format!("https://studio.apollographql.com/graph/{graph_id}/launches/{launch_id}")
}

fn build_response(publish_response: UpdateResponse) -> SubgraphPublishResponse {
    let build_errors: BuildErrors = publish_response
        .errors
        .iter()
        .filter_map(|error| {
            error.as_ref().map(|e| {
                BuildError::composition_error(e.code.clone(), Some(e.message.clone()), None, None)
            })
        })
        .collect();

    SubgraphPublishResponse {
        api_schema_hash: match publish_response.composition_config {
            Some(config) => Some(config.schema_hash),
            None => None,
        },
        supergraph_was_updated: publish_response.did_update_gateway,
        subgraph_was_created: publish_response.service_was_created,
        subgraph_was_updated: publish_response.service_was_updated,
        build_errors,
        launch_cli_copy: publish_response.launch_cli_copy,
        launch_url: publish_response.launch_url,
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

    #[tokio::test]
    async fn run_fails_when_downstream_launch_fails() {
        let server = MockServer::start_async().await;
        let _publish = server.mock(|when, then| {
            when.method(POST)
                .body_includes("mutation SubgraphPublishMutation")
                .body_includes("downstreamLaunchInitiation: SYNC");
            then.status(200).json_body(json!({
                "data": {
                    "graph": {
                        "publishSubgraph": {
                            "compositionConfig": { "schemaHash": "5gf564" },
                            "errors": [],
                            "didUpdateGateway": true,
                            "serviceWasCreated": false,
                            "serviceWasUpdated": true,
                            "launch": {
                                "id": "source-launch"
                            },
                            "launchUrl": "https://studio.example/launches/source-launch",
                            "launchCliCopy": "You can monitor this launch in Apollo Studio."
                        }
                    }
                }
            }));
        });
        let _launch = server.mock(|when, then| {
            when.method(POST)
                .body_includes("query SubgraphPublishLaunchStatusQuery");
            then.status(200).json_body(json!({
                "data": {
                    "graph": {
                        "variant": {
                            "launch": {
                                "id": "source-launch",
                                "graphId": "test-graph",
                                "graphVariant": "current",
                                "status": "LAUNCH_COMPLETED",
                                "downstreamLaunches": [
                                    {
                                        "id": "contract-launch",
                                        "graphId": "test-graph",
                                        "graphVariant": "contract",
                                        "status": "LAUNCH_FAILED"
                                    }
                                ]
                            }
                        }
                    }
                }
            }));
        });
        let client = StudioClient::new(
            Credential {
                api_key: "test".to_string(),
                origin: CredentialOrigin::EnvVar,
            },
            &server.url("/"),
            "test-version",
            false,
            ReqwestClient::new(),
            Duration::from_secs(1),
        );

        let result = run(
            SubgraphPublishInput {
                graph_ref: "test-graph@current".parse().unwrap(),
                subgraph: "products".to_string(),
                url: Some("https://example.com/graphql".to_string()),
                schema: "type Query { product: String }".to_string(),
                git_context: crate::shared::GitContext {
                    branch: None,
                    commit: None,
                    author: None,
                    remote_url: None,
                },
                convert_to_federated_graph: true,
                launch_poll_timeout_seconds: 0,
            },
            &client,
        )
        .await;

        match result {
            Err(RoverClientError::SubgraphPublishLaunchFailure {
                graph_ref,
                launch_id,
                failed_downstream_launches,
            }) => {
                assert_eq!(graph_ref.to_string(), "test-graph@current");
                assert_eq!(launch_id, "source-launch");
                assert_eq!(
                    failed_downstream_launches,
                    vec![FailedLaunch {
                        graph_id: "test-graph".to_string(),
                        graph_variant: "contract".to_string(),
                        launch_id: "contract-launch".to_string(),
                    }]
                );
            }
            other => panic!("expected downstream launch failure, got {other:?}"),
        }
    }

    #[test]
    fn build_response_works_with_composition_errors() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [
                {
                    "message": "[Accounts] User -> build error",
                    "code": null
                },
                null, // this is technically allowed in the types
                {
                    "message": "[Products] Product -> another one",
                    "code": "ERROR"
                }
            ],
            "didUpdateGateway": false,
            "serviceWasCreated": true,
            "serviceWasUpdated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: Some("5gf564".to_string()),
                build_errors: vec![
                    BuildError::composition_error(
                        None,
                        Some("[Accounts] User -> build error".to_string()),
                        None,
                        None
                    ),
                    BuildError::composition_error(
                        Some("ERROR".to_string()),
                        Some("[Products] Product -> another one".to_string()),
                        None,
                        None
                    )
                ]
                .into(),
                supergraph_was_updated: false,
                subgraph_was_created: true,
                subgraph_was_updated: true,
                launch_url: None,
                launch_cli_copy: None,
            }
        );
    }

    #[test]
    fn build_response_works_with_successful_composition() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [],
            "didUpdateGateway": true,
            "serviceWasCreated": true,
            "serviceWasUpdated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: Some("5gf564".to_string()),
                build_errors: BuildErrors::new(),
                supergraph_was_updated: true,
                subgraph_was_created: true,
                subgraph_was_updated: true,
                launch_url: None,
                launch_cli_copy: None,
            }
        );
    }

    // I think this case can happen when there are failures on the initial publish
    // before composing? No service hash to return, and serviceWasCreated: false
    #[test]
    fn build_response_works_with_failure_and_no_hash() {
        let json_response = json!({
            "compositionConfig": null,
            "errors": [{
                "message": "[Accounts] -> Things went really wrong",
                "code": null
            }],
            "didUpdateGateway": false,
            "serviceWasCreated": false,
            "serviceWasUpdated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: None,
                build_errors: vec![BuildError::composition_error(
                    None,
                    Some("[Accounts] -> Things went really wrong".to_string()),
                    None,
                    None
                )]
                .into(),
                supergraph_was_updated: false,
                subgraph_was_created: false,
                subgraph_was_updated: true,
                launch_url: None,
                launch_cli_copy: None,
            }
        );
    }

    #[test]
    fn build_response_works_with_successful_composition_and_launch() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [],
            "didUpdateGateway": true,
            "serviceWasCreated": true,
            "serviceWasUpdated": true,
            "launchUrl": "test.com/launchurl",
            "launchCliCopy": "You can monitor this launch in Apollo Studio: test.com/launchurl",
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: Some("5gf564".to_string()),
                build_errors: BuildErrors::new(),
                supergraph_was_updated: true,
                subgraph_was_created: true,
                subgraph_was_updated: true,
                launch_url: Some("test.com/launchurl".to_string()),
                launch_cli_copy: Some(
                    "You can monitor this launch in Apollo Studio: test.com/launchurl".to_string()
                ),
            }
        );
    }

    #[test]
    fn build_response_works_with_unmodified_subgraph() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [],
            "didUpdateGateway": false,
            "serviceWasCreated": false,
            "serviceWasUpdated": false
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: Some("5gf564".to_string()),
                build_errors: BuildErrors::new(),
                supergraph_was_updated: false,
                subgraph_was_created: false,
                subgraph_was_updated: false,
                launch_url: None,
                launch_cli_copy: None,
            }
        );
    }
}
