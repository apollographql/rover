use std::time::Duration;

use apollo_parser::Parser;
use graphql_client::*;
use rover_studio::types::GraphRef;
use rover_tower::poll_retry::PollRetryPolicy;
use tower::{Service, ServiceBuilder, ServiceExt};

use crate::{
    blocking::StudioClient,
    error::FailedLaunch,
    operations::graph::publish::{
        service::GraphPublishLaunchStatus,
        types::{ChangeSummary, FieldChanges, LaunchSnapshot, LaunchStatusInput, TypeChanges},
        GraphPublishInput, GraphPublishResponse,
    },
    shared::{DownstreamLaunch, LaunchStatus},
    RoverClientError,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_publish_mutation
pub(crate) struct GraphPublishMutation;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/publish/launch_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// A lightweight poll query for a launch (and its downstream contract-variant
/// launches) triggered by a `graph publish`.
pub(crate) struct GraphPublishLaunchStatusQuery;

/// Returns a message from apollo studio about the status of the update, and
/// a sha256 hash of the schema to be used with `schema publish`. If the
/// publish triggered any downstream contract-variant launches, polls until
/// every one of them (and the publish's own launch) reaches a terminal
/// state, failing the whole publish if any did not complete successfully.
pub async fn run(
    input: GraphPublishInput,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<GraphPublishResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let total_type_count = count_schema_types(&input.proposed_schema);
    let data = client.post::<GraphPublishMutation>(input.into()).await?;
    let publish_response = get_publish_response_from_data(data, graph_ref.clone())?;

    let maybe_launch_id = publish_response
        .publication
        .as_ref()
        .and_then(|publication| publication.variant.latest_launch.as_ref())
        .map(|launch| launch.id.clone());

    let (launch_url, downstream_launches) = if let Some(launch_id) = maybe_launch_id {
        let snapshot = poll_launch(&graph_ref, &launch_id, client, checks_timeout_seconds).await?;
        ensure_launch_succeeded(&graph_ref, &snapshot)?;
        build_launches_report(&graph_ref, snapshot)
    } else {
        (None, Vec::new())
    };

    build_response(
        publish_response,
        total_type_count,
        launch_url,
        downstream_launches,
    )
}

/// Hand-builds a Studio launch URL. `Launch` has no URL field of its own; this
/// matches the existing convention in `src/error/metadata/suggestion.rs`.
fn launch_url(graph_id: &str, launch_id: &str) -> String {
    format!("https://studio.apollographql.com/graph/{graph_id}/launches/{launch_id}")
}

/// Polls a launch (and its downstream contract-variant launches) until every
/// one of them leaves `LAUNCH_INITIATED`.
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
    let mut status_service = ServiceBuilder::new()
        .retry(PollRetryPolicy::new(
            Duration::from_secs(5),
            Duration::from_secs(checks_timeout_seconds),
            move || RoverClientError::LaunchTimeoutError {
                url: Some(url.clone()),
            },
        ))
        .service(GraphPublishLaunchStatus::new(
            client
                .studio_graphql_service()
                .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
        ));
    status_service.ready().await?.call(input).await
}

/// Fails the publish if the launch itself, or any of its downstream
/// contract-variant launches, didn't complete successfully.
fn ensure_launch_succeeded(
    graph_ref: &GraphRef,
    snapshot: &LaunchSnapshot,
) -> Result<(), RoverClientError> {
    let is_failed = |status: &LaunchStatus| matches!(status, LaunchStatus::FAILED);
    let failed_downstream_launches = snapshot
        .downstream_launches
        .iter()
        .filter(|launch| is_failed(&launch.status))
        .map(|launch| FailedLaunch {
            graph_id: launch.graph_id.clone(),
            graph_variant: launch.variant_name.clone(),
            launch_id: launch.launch_id.clone(),
        })
        .collect::<Vec<_>>();

    if is_failed(&snapshot.status) || !failed_downstream_launches.is_empty() {
        Err(RoverClientError::PublishLaunchFailure {
            graph_ref: graph_ref.clone(),
            launch_id: snapshot.launch_id.clone(),
            failed_downstream_launches,
        })
    } else {
        Ok(())
    }
}

/// Builds the (launch_url, downstream_launches) report from a finished launch
/// snapshot. Reports nothing (`None`, empty) when the publish triggered no
/// downstream contract-variant launches.
fn build_launches_report(
    graph_ref: &GraphRef,
    snapshot: LaunchSnapshot,
) -> (Option<String>, Vec<DownstreamLaunch>) {
    if snapshot.downstream_launches.is_empty() {
        return (None, Vec::new());
    }

    let url = launch_url(graph_ref.graph_id(), &snapshot.launch_id);
    let variants = snapshot
        .downstream_launches
        .into_iter()
        .map(|launch| DownstreamLaunch {
            url: launch_url(&launch.graph_id, &launch.launch_id),
            graph_id: launch.graph_id,
            variant_name: launch.variant_name,
            status: launch.status,
        })
        .collect();
    (Some(url), variants)
}

fn count_schema_types(schema: &str) -> u64 {
    use apollo_parser::cst::Definition;
    let cst = Parser::new(schema).parse();
    cst.document()
        .definitions()
        .filter(|def| {
            matches!(
                def,
                Definition::ObjectTypeDefinition(_)
                    | Definition::InputObjectTypeDefinition(_)
                    | Definition::InterfaceTypeDefinition(_)
                    | Definition::EnumTypeDefinition(_)
                    | Definition::UnionTypeDefinition(_)
                    | Definition::ScalarTypeDefinition(_)
            )
        })
        .count() as u64
}

fn get_publish_response_from_data(
    data: graph_publish_mutation::ResponseData,
    graph_ref: GraphRef,
) -> Result<graph_publish_mutation::GraphPublishMutationGraphUploadSchema, RoverClientError> {
    // then, from the response data, get .service?.upload_schema?
    let graph = data
        .graph
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    graph
        .upload_schema
        .ok_or(RoverClientError::MalformedResponse {
            null_field: "service.upload_schema".to_string(),
        })
}

fn build_response(
    publish_response: graph_publish_mutation::GraphPublishMutationGraphUploadSchema,
    total_type_count: u64,
    launch_url: Option<String>,
    downstream_launches: Vec<DownstreamLaunch>,
) -> Result<GraphPublishResponse, RoverClientError> {
    if !publish_response.success {
        let msg = format!(
            "Schema upload failed with error: {}",
            publish_response.message
        );
        return Err(RoverClientError::AdhocError { msg });
    }

    let hash = match &publish_response.publication {
        // we only want to print the first 6 chars of a hash
        Some(publication) => publication.schema.hash.clone()[..6].to_string(),
        None => {
            let msg = format!(
                "No data in response from schema publish. Failed with message: {}",
                publish_response.message
            );
            return Err(RoverClientError::AdhocError { msg });
        }
    };

    // If you publish the exact same schema as is currently published,
    // the response CODE is NO_CHANGES but under the result diff,
    // it gives you the diff for that hash (i.e., the first time it was published)
    // which very well may have changes. For this, we'll just look at the code
    // first and handle the response as if there was `None` for the diff
    let change_summary = if publish_response.code == "NO_CHANGES" {
        ChangeSummary::none()
    } else {
        let diff = publish_response
            .publication
            .ok_or_else(|| RoverClientError::MalformedResponse {
                null_field: "service.upload_schema.publication".to_string(),
            })?
            .diff_to_previous;

        if let Some(diff) = diff {
            diff.into()
        } else {
            ChangeSummary::none()
        }
    };

    Ok(GraphPublishResponse {
        api_schema_hash: hash,
        change_summary,
        total_type_count,
        launch_url,
        downstream_launches,
    })
}

type QueryChangeDiff =
    graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationDiffToPrevious;

impl From<QueryChangeDiff> for ChangeSummary {
    fn from(input: QueryChangeDiff) -> Self {
        Self {
            field_changes: input.change_summary.field.into(),
            type_changes: input.change_summary.type_.into(),
        }
    }
}

type QueryFieldChanges =
graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationDiffToPreviousChangeSummaryField;

impl From<QueryFieldChanges> for FieldChanges {
    fn from(input: QueryFieldChanges) -> Self {
        Self::with_diff(
            input.additions as u64,
            input.removals as u64,
            input.edits as u64,
        )
    }
}

type QueryTypeChanges =
graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationDiffToPreviousChangeSummaryType;

impl From<QueryTypeChanges> for TypeChanges {
    fn from(input: QueryTypeChanges) -> Self {
        Self::with_diff(
            input.additions as u64,
            input.removals as u64,
            input.edits as u64,
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::operations::graph::publish::types::DownstreamLaunchSnapshot;

    #[test]
    fn count_schema_types_counts_named_type_definitions() {
        let schema = r#"
            type Query { hello: String }
            type Foo { id: ID! }
            input CreateFooInput { name: String! }
            interface Node { id: ID! }
            enum Status { ACTIVE INACTIVE }
            union SearchResult = Foo
            scalar DateTime
            extend type Foo { extra: String }
        "#;
        assert_eq!(count_schema_types(schema), 7);
    }

    #[test]
    fn count_schema_types_returns_zero_for_empty_schema() {
        assert_eq!(count_schema_types(""), 0);
    }

    #[test]
    fn get_publish_response_from_data_gets_data() {
        let json_response = json!({
            "graph": {
                "uploadSchema": {
                    "code": "IT_WERK",
                    "message": "it really do be published",
                    "success": true,
                    "publication": {
                        "variant": { "name": "current" },
                        "schema": { "hash": "123456" }
                    }
                }
            }
        });
        let data: graph_publish_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_publish_response_from_data(data, mock_graph_ref());

        assert!(output.is_ok());
        assert_eq!(
            output.unwrap(),
            graph_publish_mutation::GraphPublishMutationGraphUploadSchema {
                code: "IT_WERK".to_string(),
                message: "it really do be published".to_string(),
                success: true,
                publication: Some(graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublication {
                    variant: graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationVariant {
                        name: "current".to_string(),
                        latest_launch: None,
                    },
                    schema: graph_publish_mutation::GraphPublishMutationGraphUploadSchemaPublicationSchema {
                        hash: "123456".to_string()
                    },
                    diff_to_previous: None,
                }),
            }
        );
    }

    #[test]
    fn get_publish_response_from_data_errs_with_no_service() {
        let json_response = json!({ "service": null });
        let data: graph_publish_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_publish_response_from_data(data, mock_graph_ref());

        assert!(output.is_err());
    }

    #[test]
    fn get_publish_response_from_data_errs_with_no_upload_response() {
        let json_response = json!({
            "graph": {
                "uploadSchema": null
            }
        });
        let data: graph_publish_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_publish_response_from_data(data, mock_graph_ref());

        assert!(output.is_err());
    }

    #[test]
    fn build_response_struct_from_success() {
        let json_response = json!({
            "code": "IT_WERK",
            "message": "it really do be published",
            "success": true,
            "publication": {
                "variant": { "name": "current" },
                "schema": { "hash": "123456" }
            }
        });
        let update_response: graph_publish_mutation::GraphPublishMutationGraphUploadSchema =
            serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response, 5, None, Vec::new());

        assert!(output.is_ok());
        assert_eq!(
            output.unwrap(),
            GraphPublishResponse {
                api_schema_hash: "123456".to_string(),
                change_summary: ChangeSummary::none(),
                total_type_count: 5,
                launch_url: None,
                downstream_launches: Vec::new(),
            }
        );
    }

    #[test]
    fn build_response_errs_when_unsuccessful() {
        let json_response = json!({
            "code": "BAD_JOB",
            "message": "it really do be like that sometime",
            "success": false,
            "tag": null
        });
        let update_response: graph_publish_mutation::GraphPublishMutationGraphUploadSchema =
            serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response, 0, None, Vec::new());

        assert!(output.is_err());
    }

    #[test]
    fn build_response_errs_when_no_tag() {
        let json_response = json!({
            "code": "BAD_JOB",
            "message": "it really do be like that sometime",
            "success": true,
            "tag": null
        });
        let update_response: graph_publish_mutation::GraphPublishMutationGraphUploadSchema =
            serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response, 0, None, Vec::new());

        assert!(output.is_err());
    }

    #[test]
    fn build_change_summary_works_with_changes() {
        let json_diff = json!({
            "changeSummary": {
                "type": {
                "additions": 4,
                "removals": 0,
                "edits": 2
                },
                "field": {
                "additions": 3,
                "removals": 1,
                "edits": 0
                }
            }
        });
        let diff_to_previous: QueryChangeDiff = serde_json::from_value(json_diff).unwrap();
        let output: ChangeSummary = diff_to_previous.into();
        assert_eq!(
            output.to_string(),
            "[Fields: +3 -1 △ 0, Types: +4 -0 △ 2]".to_string()
        )
    }

    #[test]
    fn build_change_summary_works_with_no_changes() {
        assert_eq!(
            ChangeSummary::none().to_string(),
            "[No Changes]".to_string()
        )
    }

    fn mock_graph_ref() -> GraphRef {
        GraphRef::new("mygraph", Some("current")).unwrap()
    }

    fn snapshot_with(
        status: LaunchStatus,
        downstream: Vec<(&str, &str, LaunchStatus)>,
    ) -> LaunchSnapshot {
        LaunchSnapshot {
            launch_id: "launch-1".to_string(),
            graph_id: "mygraph".to_string(),
            status,
            downstream_launches: downstream
                .into_iter()
                .map(
                    |(graph_id, variant_name, status)| DownstreamLaunchSnapshot {
                        launch_id: format!("{graph_id}-launch"),
                        graph_id: graph_id.to_string(),
                        variant_name: variant_name.to_string(),
                        status,
                    },
                )
                .collect(),
        }
    }

    #[test]
    fn ensure_launch_succeeded_ok_when_everything_completed() {
        let snapshot = snapshot_with(
            LaunchStatus::COMPLETED,
            vec![
                ("mygraph", "mobile", LaunchStatus::COMPLETED),
                ("mygraph", "partner-api", LaunchStatus::COMPLETED),
            ],
        );

        assert!(ensure_launch_succeeded(&mock_graph_ref(), &snapshot).is_ok());
    }

    #[test]
    fn ensure_launch_succeeded_errs_when_source_launch_failed() {
        let snapshot = snapshot_with(LaunchStatus::FAILED, vec![]);

        let err = ensure_launch_succeeded(&mock_graph_ref(), &snapshot).unwrap_err();
        assert_eq!(
            err.to_string(),
            "The publish for 'mygraph@current' succeeded, but launch 'launch-1' failed."
        );
    }

    #[test]
    fn ensure_launch_succeeded_errs_when_a_downstream_launch_failed() {
        let snapshot = snapshot_with(
            LaunchStatus::COMPLETED,
            vec![
                ("mygraph", "mobile", LaunchStatus::COMPLETED),
                ("mygraph", "partner-api", LaunchStatus::FAILED),
            ],
        );

        let err = ensure_launch_succeeded(&mock_graph_ref(), &snapshot).unwrap_err();
        assert_eq!(
            err.to_string(),
            "The publish for 'mygraph@current' succeeded, but downstream contract launch(es) failed: mygraph@partner-api (launch mygraph-launch)."
        );
    }

    #[test]
    fn build_launches_report_returns_nothing_when_no_downstream_launches() {
        let snapshot = snapshot_with(LaunchStatus::COMPLETED, vec![]);

        assert_eq!(
            build_launches_report(&mock_graph_ref(), snapshot),
            (None, Vec::new())
        );
    }

    #[test]
    fn build_launches_report_returns_variants_with_urls() {
        let snapshot = snapshot_with(
            LaunchStatus::COMPLETED,
            vec![("mygraph", "mobile", LaunchStatus::COMPLETED)],
        );

        let (url, variants) = build_launches_report(&mock_graph_ref(), snapshot);
        assert_eq!(
            url,
            Some("https://studio.apollographql.com/graph/mygraph/launches/launch-1".to_string())
        );
        assert_eq!(
            variants,
            vec![DownstreamLaunch {
                graph_id: "mygraph".to_string(),
                variant_name: "mobile".to_string(),
                status: LaunchStatus::COMPLETED,
                url: "https://studio.apollographql.com/graph/mygraph/launches/mygraph-launch"
                    .to_string(),
            }]
        );
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

    fn test_input() -> GraphPublishInput {
        GraphPublishInput {
            graph_ref: mock_graph_ref(),
            proposed_schema: "type Query { hello: String }".to_string(),
            git_context: crate::shared::GitContext {
                branch: None,
                commit: None,
                author: None,
                remote_url: None,
            },
            changelog_message: None,
        }
    }

    #[tokio::test]
    async fn run_reports_no_launches_when_publish_has_no_launch() {
        use httpmock::prelude::*;

        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).body_includes("GraphPublishMutation");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "uploadSchema": {
                    "code": "IT_WERK",
                    "message": "published",
                    "success": true,
                    "publication": {
                        "variant": { "name": "current", "latestLaunch": null },
                        "schema": { "hash": "123456" }
                    }
                } } }
            }));
        });

        let response = run(test_input(), &test_client(&server.url("/")), 30)
            .await
            .unwrap();

        assert_eq!(response.launch_url, None);
        assert_eq!(response.downstream_launches, Vec::new());
    }

    #[tokio::test]
    async fn run_polls_and_reports_triggered_downstream_launches() {
        use httpmock::prelude::*;

        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).body_includes("GraphPublishMutation");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "uploadSchema": {
                    "code": "IT_WERK",
                    "message": "published",
                    "success": true,
                    "publication": {
                        "variant": { "name": "current", "latestLaunch": { "id": "launch-1" } },
                        "schema": { "hash": "123456" }
                    }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("GraphPublishLaunchStatusQuery");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "variant": { "launch": {
                    "id": "launch-1",
                    "graphId": "mygraph",
                    "graphVariant": "current",
                    "status": "LAUNCH_COMPLETED",
                    "downstreamLaunches": [
                        {
                            "id": "launch-2",
                            "graphId": "mygraph",
                            "graphVariant": "mobile",
                            "status": "LAUNCH_COMPLETED"
                        }
                    ]
                } } } }
            }));
        });

        let response = run(test_input(), &test_client(&server.url("/")), 30)
            .await
            .unwrap();

        assert_eq!(
            response.launch_url,
            Some("https://studio.apollographql.com/graph/mygraph/launches/launch-1".to_string())
        );
        assert_eq!(
            response.downstream_launches,
            vec![DownstreamLaunch {
                graph_id: "mygraph".to_string(),
                variant_name: "mobile".to_string(),
                status: LaunchStatus::COMPLETED,
                url: "https://studio.apollographql.com/graph/mygraph/launches/launch-2".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn run_fails_when_a_downstream_launch_fails() {
        use httpmock::prelude::*;

        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).body_includes("GraphPublishMutation");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "uploadSchema": {
                    "code": "IT_WERK",
                    "message": "published",
                    "success": true,
                    "publication": {
                        "variant": { "name": "current", "latestLaunch": { "id": "launch-1" } },
                        "schema": { "hash": "123456" }
                    }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("GraphPublishLaunchStatusQuery");
            then.status(200).json_body(serde_json::json!({
                "data": { "graph": { "variant": { "launch": {
                    "id": "launch-1",
                    "graphId": "mygraph",
                    "graphVariant": "current",
                    "status": "LAUNCH_COMPLETED",
                    "downstreamLaunches": [
                        {
                            "id": "launch-2",
                            "graphId": "mygraph",
                            "graphVariant": "mobile",
                            "status": "LAUNCH_FAILED"
                        }
                    ]
                } } } }
            }));
        });

        let err = run(test_input(), &test_client(&server.url("/")), 30)
            .await
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The publish for 'mygraph@current' succeeded, but downstream contract launch(es) failed: mygraph@mobile (launch launch-2)."
        );
    }
}
