use std::time::Duration;

use apollo_federation_types::rover::{BuildError, BuildErrors};
use graphql_client::*;
use rover_studio::types::GraphRef;
use rover_tower::poll_retry::poll_until_complete;
use tower::{Service, ServiceExt};

use super::{service::SubgraphPublishLaunchStatus, types::*};
use crate::{
    blocking::StudioClient,
    operations::{
        config::is_federated::{self, IsFederatedInput},
        graph::{variant, variant::VariantListInput},
    },
    shared::{DownstreamLaunch, LaunchStatus},
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

/// Publishes a subgraph schema. If the publish triggered a launch and
/// `launch_poll_timeout_seconds` is `Some`, polls until it (and every
/// downstream contract-variant launch it triggered) reaches a terminal
/// state. The outcome -- including a launch or downstream launch that ended
/// `LAUNCH_FAILED` -- is reported as data on the returned response, not as
/// an `Err`: the subgraph publish itself already succeeded by this point,
/// matching `graph publish`'s approach.
///
/// `launch_poll_timeout_seconds: None` skips polling entirely (used by
/// `rover init`, which publishes several subgraphs to the same variant in a
/// row and doesn't need to wait on -- or even look at -- any of their
/// launches, only the last of which wouldn't even be superseded).
pub async fn run(
    input: SubgraphPublishInput,
    client: &StudioClient,
    launch_poll_timeout_seconds: Option<u64>,
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

    let maybe_launch_id = publish_response
        .launch
        .as_ref()
        .map(|launch| launch.id.clone());

    let (launch_status, launch_superseded, downstream_launches) =
        match (maybe_launch_id, launch_poll_timeout_seconds) {
            (Some(launch_id), Some(timeout_seconds)) => {
                let snapshot = poll_launch(&graph_ref, &launch_id, client, timeout_seconds).await?;
                build_launches_report(snapshot)
            }
            _ => (None, false, Vec::new()),
        };

    Ok(build_response(
        publish_response,
        launch_status,
        launch_superseded,
        downstream_launches,
    ))
}

/// Hand-builds a Studio launch URL for a downstream contract-variant launch.
/// The source launch's own URL comes directly from the mutation's
/// `launchUrl` field instead -- unlike `graph publish`, `subgraph publish`'s
/// mutation already returns it.
fn launch_url(graph_id: &str, launch_id: &str) -> String {
    format!("https://studio.apollographql.com/graph/{graph_id}/launches/{launch_id}")
}

/// Polls a launch (and its downstream contract-variant launches) until every
/// one of them leaves `LAUNCH_INITIATED`. A launch that isn't visible yet
/// (`LaunchPoll::NotFound` -- expected immediately after the mutation that
/// created it, before Studio's read path catches up) keeps the loop going
/// rather than failing; if it's still not visible once `checks_timeout_seconds`
/// elapses, this surfaces as a `LaunchTimeoutError` like any other launch
/// that never finished.
async fn poll_launch(
    graph_ref: &GraphRef,
    launch_id: &str,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<LaunchSnapshot, RoverClientError> {
    let url = launch_url(graph_ref.graph_id(), launch_id);
    let input = LaunchStatusInput {
        graph_ref: graph_ref.clone(),
        launch_id: launch_id.to_string(),
    };
    let mut status_service = poll_until_complete(
        SubgraphPublishLaunchStatus::new(
            client
                .studio_graphql_service()
                .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
        ),
        Duration::from_secs(5),
        Duration::from_secs(checks_timeout_seconds),
        move || RoverClientError::LaunchTimeoutError {
            url: Some(url.clone()),
        },
    );
    match status_service.ready().await?.call(input).await? {
        LaunchPoll::Found(snapshot) => Ok(snapshot),
        // Unreachable in practice: `LaunchPoll::NotFound` always reports
        // `SimplePollOutcome::Incomplete`, so `poll_until_complete` never
        // returns it as a final `Ok` -- it either keeps polling or times out
        // above. Kept as a non-panicking fallback in case that invariant
        // ever changes.
        LaunchPoll::NotFound => Err(RoverClientError::LaunchNotFound {
            graph_ref: graph_ref.clone(),
            launch_id: launch_id.to_string(),
        }),
    }
}

/// Builds the (launch_status, launch_superseded, downstream_launches) report
/// from a finished launch snapshot. Always populated once a launch was
/// polled -- including a `FAILED` status, which is reported as data here
/// rather than as an error (see `run`'s doc comment). A superseded launch
/// keeps `status == INITIATED` forever per the API; `superseded` is the only
/// way to tell it apart from one still genuinely in flight.
fn build_launches_report(
    snapshot: LaunchSnapshot,
) -> (Option<LaunchStatus>, bool, Vec<DownstreamLaunch>) {
    let variants = snapshot
        .downstream_launches
        .into_iter()
        .map(|launch| DownstreamLaunch {
            url: launch_url(&launch.graph_id, &launch.launch_id),
            graph_id: launch.graph_id,
            variant_name: launch.variant_name,
            status: launch.status,
            superseded: launch.superseded,
        })
        .collect();
    (Some(snapshot.status), snapshot.superseded, variants)
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

fn build_response(
    publish_response: UpdateResponse,
    launch_status: Option<LaunchStatus>,
    launch_superseded: bool,
    downstream_launches: Vec<DownstreamLaunch>,
) -> SubgraphPublishResponse {
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
        launch_status,
        launch_superseded,
        downstream_launches,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use speculoos::prelude::*;

    use super::*;
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
        let output = build_response(update_response, None, false, Vec::new());

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
                launch_status: None,
                launch_superseded: false,
                downstream_launches: Vec::new(),
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
        let output = build_response(update_response, None, false, Vec::new());

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
                launch_status: None,
                launch_superseded: false,
                downstream_launches: Vec::new(),
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
        let output = build_response(update_response, None, false, Vec::new());

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
                launch_status: None,
                launch_superseded: false,
                downstream_launches: Vec::new(),
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
        let output = build_response(update_response, None, false, Vec::new());

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
                launch_status: None,
                launch_superseded: false,
                downstream_launches: Vec::new(),
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
        let output = build_response(update_response, None, false, Vec::new());

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
                launch_status: None,
                launch_superseded: false,
                downstream_launches: Vec::new(),
            }
        );
    }

    fn mock_graph_ref() -> GraphRef {
        GraphRef::new("mygraph", Some("current")).unwrap()
    }

    fn test_client(server_url: &str) -> StudioClient {
        use std::time::Duration as StdDuration;

        use houston::{Credential, CredentialOrigin};
        use reqwest::Client as ReqwestClient;

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
            StdDuration::from_secs(1),
        )
    }

    fn test_input() -> SubgraphPublishInput {
        SubgraphPublishInput {
            graph_ref: mock_graph_ref(),
            subgraph: "subgraph".to_string(),
            url: Some("http://example.com".to_string()),
            schema: "type Query { hello: String }".to_string(),
            git_context: crate::shared::GitContext {
                branch: None,
                commit: None,
                author: None,
                remote_url: None,
            },
            convert_to_federated_graph: false,
            changelog_message: None,
        }
    }

    fn mutation_mock_with_launch(server: &httpmock::MockServer) {
        use httpmock::prelude::*;

        server.mock(|when, then| {
            when.method(POST).body_includes("SubgraphPublishMutation");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "publishSubgraph": {
                    "compositionConfig": { "schemaHash": "5gf564" },
                    "errors": [],
                    "didUpdateGateway": true,
                    "serviceWasCreated": false,
                    "serviceWasUpdated": true,
                    "launch": { "id": "launch-1" },
                    "launchUrl": "https://studio.apollographql.com/graph/mygraph/launches/launch-1",
                    "launchCliCopy": null
                } } }
            }));
        });
    }

    #[tokio::test]
    async fn run_reports_no_launches_when_publish_has_no_launch() {
        use httpmock::prelude::*;

        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).body_includes("SubgraphPublishMutation");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "publishSubgraph": {
                    "compositionConfig": { "schemaHash": "5gf564" },
                    "errors": [],
                    "didUpdateGateway": true,
                    "serviceWasCreated": false,
                    "serviceWasUpdated": true,
                    "launch": null,
                    "launchUrl": null,
                    "launchCliCopy": null
                } } }
            }));
        });

        let response = run(test_input(), &test_client(&server.url("/")), Some(30))
            .await
            .unwrap();

        assert_that!(response.launch_status).is_none();
        assert_that!(response.downstream_launches).is_equal_to(Vec::new());
    }

    /// `None` skips polling entirely regardless of whether the mutation
    /// triggered a launch -- used by `rover init`, which doesn't need launch
    /// data and shouldn't block on it. No `SubgraphPublishLaunchStatusQuery`
    /// mock is registered, so a regression that polls anyway would fail this
    /// test via an unmatched request.
    #[tokio::test]
    async fn run_skips_polling_when_launch_poll_timeout_seconds_is_none() {
        let server = httpmock::MockServer::start_async().await;
        mutation_mock_with_launch(&server);

        let response = run(test_input(), &test_client(&server.url("/")), None)
            .await
            .unwrap();

        assert_that!(response.launch_status).is_none();
        assert_that!(response.launch_superseded).is_false();
        assert_that!(response.downstream_launches).is_equal_to(Vec::new());
    }

    #[tokio::test]
    async fn run_polls_and_reports_triggered_downstream_launches() {
        use httpmock::prelude::*;

        let server = MockServer::start_async().await;
        mutation_mock_with_launch(&server);
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("SubgraphPublishLaunchStatusQuery");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "variant": { "launch": {
                    "id": "launch-1",
                    "graphId": "mygraph",
                    "graphVariant": "current",
                    "status": "LAUNCH_COMPLETED",
                    "supersededAt": null,
                    "downstreamLaunches": [
                        {
                            "id": "launch-2",
                            "graphId": "mygraph",
                            "graphVariant": "mobile",
                            "status": "LAUNCH_COMPLETED",
                            "supersededAt": null
                        }
                    ]
                } } } }
            }));
        });

        let response = run(test_input(), &test_client(&server.url("/")), Some(30))
            .await
            .unwrap();

        assert_that!(response.launch_status).is_equal_to(Some(LaunchStatus::COMPLETED));
        assert_that!(response.downstream_launches).is_equal_to(vec![DownstreamLaunch {
            graph_id: "mygraph".to_string(),
            variant_name: "mobile".to_string(),
            status: LaunchStatus::COMPLETED,
            superseded: false,
            url: "https://studio.apollographql.com/graph/mygraph/launches/launch-2".to_string(),
        }]);
    }

    /// A failed downstream launch is reported as data (not an `Err`) -- the
    /// subgraph publish itself already succeeded by the time this is known.
    #[tokio::test]
    async fn run_reports_a_failed_downstream_launch_as_data() {
        use httpmock::prelude::*;

        let server = MockServer::start_async().await;
        mutation_mock_with_launch(&server);
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("SubgraphPublishLaunchStatusQuery");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "variant": { "launch": {
                    "id": "launch-1",
                    "graphId": "mygraph",
                    "graphVariant": "current",
                    "status": "LAUNCH_COMPLETED",
                    "supersededAt": null,
                    "downstreamLaunches": [
                        {
                            "id": "launch-2",
                            "graphId": "mygraph",
                            "graphVariant": "mobile",
                            "status": "LAUNCH_FAILED",
                            "supersededAt": null
                        }
                    ]
                } } } }
            }));
        });

        let response = run(test_input(), &test_client(&server.url("/")), Some(30))
            .await
            .unwrap();

        assert_that!(response.launch_status).is_equal_to(Some(LaunchStatus::COMPLETED));
        assert_that!(response.downstream_launches).is_equal_to(vec![DownstreamLaunch {
            graph_id: "mygraph".to_string(),
            variant_name: "mobile".to_string(),
            status: LaunchStatus::FAILED,
            superseded: false,
            url: "https://studio.apollographql.com/graph/mygraph/launches/launch-2".to_string(),
        }]);
    }

    /// A launch that gets superseded by a later concurrent publish keeps
    /// `status == LAUNCH_INITIATED` forever per the schema -- `run` must
    /// treat that as terminal (not spin until `LaunchTimeoutError`), and
    /// report the supersession as data.
    #[tokio::test]
    async fn run_reports_a_superseded_launch_as_data_instead_of_timing_out() {
        use httpmock::prelude::*;

        let server = MockServer::start_async().await;
        mutation_mock_with_launch(&server);
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("SubgraphPublishLaunchStatusQuery");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "variant": { "launch": {
                    "id": "launch-1",
                    "graphId": "mygraph",
                    "graphVariant": "current",
                    "status": "LAUNCH_INITIATED",
                    "supersededAt": "2024-01-01T00:00:00Z",
                    "downstreamLaunches": []
                } } } }
            }));
        });

        let response = run(test_input(), &test_client(&server.url("/")), Some(30))
            .await
            .unwrap();

        assert_that!(response.launch_status).is_equal_to(Some(LaunchStatus::INITIATED));
        assert_that!(response.launch_superseded).is_true();
    }
}
