use std::fmt::Debug;

use apollo_federation_types::rover::BuildErrors;
use itertools::Itertools;
use rover_graphql::GraphQLServiceError;
use thiserror::Error;

use crate::shared::{CheckTaskStatus, CheckWorkflowResponse, GraphRef, LintResponse};

/// RoverClientError represents all possible failures that can occur during a client request.
#[derive(Error, Debug)]
pub enum RoverClientError {
    /// The provided GraphQL was invalid.
    #[error("{msg}")]
    GraphQl {
        /// The encountered GraphQL error.
        msg: String,
    },

    /// Failed to parse Introspection Response coming from server.
    #[error("{msg}")]
    IntrospectionError {
        /// Introspection Error coming from schema encoder.
        msg: String,
    },

    /// Tried to build a [`HeaderMap`] with an invalid header name.
    #[error("Invalid header name")]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),

    /// Tried to build a [`HeaderMap`] with an invalid header value.
    #[error("Invalid header value")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    /// Invalid JSON in response body.
    #[error("Could not parse JSON")]
    InvalidJson(#[from] serde_json::Error),

    /// Encountered an error handling the received response.
    #[error("{msg}")]
    AdhocError {
        /// The error message.
        msg: String,
    },

    /// Encountered a 400-599 error from an endpoint.
    #[error("Unable to get a response from an endpoint. Client returned an error.\n\n{msg}")]
    ClientError {
        /// Error message from client.
        msg: String,
    },

    /// when a graph does not have an account associated with it.
    #[error("Could not find organization associated with graph \"{graph_id}\"")]
    OrganizationNotFound { graph_id: String },

    /// The user provided an invalid subgraph name.
    #[error("Could not find subgraph \"{invalid_subgraph}\".")]
    NoSubgraphInGraph {
        /// The invalid subgraph name
        invalid_subgraph: String,

        /// A list of valid subgraph names
        // this is not used in the error message, but can be accessed
        // by application-level error handlers
        valid_subgraphs: Vec<String>,
    },

    /// The Studio API could not find a variant for a graph
    #[error(
        "The graph registry does not contain variant \"{}\" for graph \"{}\"", graph_ref.variant, graph_ref.name
    )]
    NoSchemaForVariant {
        /// The graph ref.
        graph_ref: GraphRef,

        /// Valid variants.
        valid_variants: Vec<String>,

        /// Front end URL root.
        frontend_url_root: String,
    },

    /// Encountered an error sending the request.
    #[error("{}", source)]
    SendRequest {
        source: reqwest::Error,
        endpoint_kind: EndpointKind,
    },

    /// when someone provides a bad graph/variant combination or isn't
    /// validated properly, we don't know which reason is at fault for data.service
    /// being empty, so this error tells them to check both.
    #[error("Could not find graph with name \"{graph_ref}\"")]
    GraphNotFound { graph_ref: GraphRef },

    /// when someone provides a graph ID that doesn't exist.
    #[error("Could not find graph with ID \"{graph_id}\"")]
    GraphIdNotFound { graph_id: String },

    /// if someone attempts to get a core schema from a supergraph that has
    /// no successful build in the API, we return this error.
    #[error("No supergraph SDL exists for \"{graph_ref}\" because its subgraphs failed to build.")]
    NoSupergraphBuilds {
        graph_ref: GraphRef,
        source: BuildErrors,
    },

    #[error("Encountered {} while trying to build a supergraph.", .source.length_string())]
    BuildErrors {
        source: BuildErrors,
        num_subgraphs: usize,
    },

    #[error("Encountered {} while trying to build subgraph \"{subgraph}\" into supergraph \"{graph_ref}\".", .source.length_string())]
    SubgraphBuildErrors {
        subgraph: String,
        graph_ref: GraphRef,
        source: BuildErrors,
    },

    #[error("{}", contract_publish_errors_msg(.msgs, .no_launch))]
    ContractPublishErrors { msgs: Vec<String>, no_launch: bool },

    /// This error occurs when the Studio API returns no implementing services for a graph
    /// This response shouldn't be possible!
    #[error("The response from Apollo Studio was malformed. Response body contains `null` value for \"{null_field}\"")]
    MalformedResponse { null_field: String },

    /// This error occurs when an operation expected a federated graph but a non-federated
    /// graph was supplied.
    /// `can_operation_convert` is only set to true when a non-federated graph
    /// was encountered during an operation that could potentially convert a non-federated graph
    /// to a federated graph.
    #[error("The graph `{graph_ref}` is a non-federated graph. This operation is only possible for federated graphs.")]
    ExpectedFederatedGraph {
        graph_ref: GraphRef,
        can_operation_convert: bool,
    },

    /// This error occurs when an operation expected a contract variant but a non-contract variant
    /// was supplied.
    #[error("The variant `{graph_ref}` is a non-contract variant. This operation is only possible for contract variants.")]
    ExpectedContractVariant { graph_ref: GraphRef },

    /// The API returned an invalid ChangeSeverity value
    #[error("Invalid ChangeSeverity.")]
    InvalidSeverity,

    /// The user supplied an invalid validation period
    #[error("You can only specify a duration as granular as seconds.")]
    ValidationPeriodTooGranular,

    /// The user supplied an invalid validation period duration
    #[error(transparent)]
    InvalidValidationPeriodDuration(#[from] humantime::DurationError),

    /// This error occurs when a user proposes a schema that cause checks to fail.
    #[error("{}", check_workflow_error_msg(.check_response))]
    CheckWorkflowFailure {
        graph_ref: GraphRef,
        check_response: Box<CheckWorkflowResponse>,
    },

    /// While linting the proposed schema, some rule violations were found
    #[error("While linting the proposed schema, some rule violations were found")]
    LintFailures { lint_response: LintResponse },

    /// Encountered errors while converting a persisted query manifest generated by the Relay compiler to the structure
    /// required by Apollo GraphOS
    #[error("The persisted query manifest generated by the Relay compiler contained the following errors:\n\n{errors}")]
    RelayOperationParseFailures { errors: String },

    /// This error occurs when a user has a malformed Graph Ref
    #[error("Graph IDs must be in the format <NAME> or <NAME>@<VARIANT>, where <NAME> can only contain letters, numbers, or the characters `-` or `_`, and must be 64 characters or less. <VARIANT> must be 64 characters or less.")]
    InvalidGraphRef,

    /// This error occurs when a user has a malformed API key
    #[error(
        "The API key you provided is malformed. An API key must have three parts separated by a colon."
    )]
    MalformedKey,

    /// The registry could not find this key
    #[error("The registry did not recognize the provided API key")]
    InvalidKey,

    /// Could not parse the latest version
    #[error("Could not parse the latest release version")]
    UnparseableReleaseVersion { source: semver::Error },

    /// Encountered an error while processing the request for the latest version
    #[error("There's something wrong with the latest GitHub release URL")]
    BadReleaseUrl,

    #[error("This endpoint doesn't support subgraph introspection via the Query._service field")]
    SubgraphIntrospectionNotAvailable,

    #[error("The input provided is invalid")]
    InvalidInputError { graph_ref: GraphRef },

    #[error("You don't have the required permissions to perform this operation: {msg}.")]
    PermissionError { msg: String },

    #[error(
        "You cannot perform this operation due to a limit imposed by your current billing plan"
    )]
    PlanError { msg: String },

    #[error("The check workflow took too long to run.")]
    ChecksTimeoutError { url: Option<String> },

    #[error(
        "A check workflow status was reported but it was not specified as a pass or a failure."
    )]
    UnknownCheckWorkflowStatus,

    #[error("You cannot publish a new subgraph without specifying a routing URL.")]
    MissingRoutingUrlError {
        subgraph_name: String,
        graph_ref: GraphRef,
    },

    #[error("Could not find a persisted query list linked to {graph_ref}.")]
    NoPersistedQueryList {
        graph_ref: GraphRef,
        frontend_url_root: String,
    },

    #[error("Could not find a persisted query list with ID '{list_id}' associated with the '{graph_id}' graph.")]
    PersistedQueryListIdNotFound {
        graph_id: String,
        list_id: String,
        frontend_url_root: String,
    },

    #[error("Offline licences are not enabled for your organization.")]
    OfflineLicenseNotEnabled,

    #[error("You've encountered a rate limit.")]
    RateLimitExceeded,

    #[error("Invalid router config: {msg}")]
    InvalidRouterConfig { msg: String },

    #[error("Cannot operate on a non-cloud graph ref {graph_ref}")]
    NonCloudGraphRef { graph_ref: GraphRef },

    #[error("Service failed to become ready")]
    ServiceReady(Box<dyn std::error::Error + Send + Sync>),

    #[error("{}", .source)]
    Service {
        source: Box<dyn std::error::Error + Send + Sync>,
        endpoint_kind: EndpointKind,
    },
}

fn contract_publish_errors_msg(msgs: &[String], no_launch: &bool) -> String {
    let plural = match msgs.len() {
        1 => "",
        _ => "s",
    };
    let maybe_launch = if !no_launch {
        " and triggering launch"
    } else {
        ""
    };
    format!(
        "While publishing the contract configuration{}, the following error{} occurred:\n{}",
        maybe_launch,
        plural,
        msgs.join("\n"),
    )
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum EndpointKind {
    ApolloStudio,
    Customer,
    Orbiter,
}

fn check_workflow_error_msg(check_response: &CheckWorkflowResponse) -> String {
    let failed_tasks: Vec<&str> = [
        if let Some(operations_response) = &check_response.maybe_operations_response {
            if operations_response.task_status == CheckTaskStatus::FAILED {
                Some("operation")
            } else {
                None
            }
        } else {
            None
        },
        if let Some(lint_response) = &check_response.maybe_lint_response {
            if lint_response.task_status == CheckTaskStatus::FAILED {
                Some("linter")
            } else {
                None
            }
        } else {
            None
        },
        if let Some(downstream_response) = &check_response.maybe_downstream_response {
            if downstream_response.task_status == CheckTaskStatus::FAILED {
                Some("downstream")
            } else {
                None
            }
        } else {
            None
        },
        if let Some(proposals_response) = &check_response.maybe_proposals_response {
            if proposals_response.task_status == CheckTaskStatus::FAILED {
                Some("proposal")
            } else {
                None
            }
        } else {
            None
        },
        if let Some(custom_response) = &check_response.maybe_custom_response {
            if custom_response.task_status == CheckTaskStatus::FAILED {
                Some("custom")
            } else {
                None
            }
        } else {
            None
        },
    ]
    .iter()
    .filter_map(|&x| x)
    .collect();

    match failed_tasks.as_slice() {
        [] => "The changes in the schema you proposed resulted in an unknown check task to fail."
            .to_string(),
        [single_task] => format!(
            "The changes in the schema you proposed caused {} checks to fail.",
            single_task
        ),
        tasks => {
            let (all_but_last, last) = tasks.split_at(tasks.len() - 1);
            let all_but_last = all_but_last.join(", ");
            format!(
                "The changes in the schema you proposed caused {} and {} checks to fail.",
                all_but_last, last[0]
            )
        }
    }
}

impl<T: Debug + Send + Sync> From<GraphQLServiceError<T>> for RoverClientError {
    fn from(value: GraphQLServiceError<T>) -> Self {
        match value {
            GraphQLServiceError::NoData(_) => RoverClientError::GraphQl {
                msg: value.to_string(),
            },
            GraphQLServiceError::PartialError { errors, .. } => {
                let errors = errors.iter().map(|err| err.to_string()).join("\n");
                RoverClientError::GraphQl {
                    msg: format!("Response returned with errors:\n{}", errors),
                }
            }
            _ => RoverClientError::ClientError {
                msg: value.to_string(),
            },
        }
    }
}
