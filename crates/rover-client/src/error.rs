use std::fmt::Debug;

use apollo_federation_types::rover::BuildErrors;
use itertools::Itertools;
use rover_graphql::GraphQLServiceError;
use rover_http::HttpServiceError;
use rover_studio::{
    service::{
        permission_denied::permission_denied,
        rejected_credential::{rejected_credential, RejectedCredential},
    },
    types::{GraphRef, InvalidGraphRef},
};
use thiserror::Error;

use crate::{
    operations::api_key::list::ApiKey,
    shared::{CheckTaskStatus, CheckWorkflowResponse, LintResponse},
};

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

    /// Invalid Timestamp in response body
    #[error("Could not parse Timestamp")]
    InvalidTimestamp(#[from] chrono::ParseError),

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
    #[error("Could not find organization associated with graph '{graph_id}'")]
    OrganizationNotFound { graph_id: String },

    /// when attempting to create a key the associated Organization cannot be found
    #[error("Could not find organization with ID '{organization_id}'")]
    OrganizationIDNotFound { organization_id: String },

    /// The Platform API refused a client-credential pair mutation (create/rotate/delete - the
    /// operations composed over
    /// [`crate::blocking::StudioClient::studio_graphql_service_with_timeout`], the only
    /// constructor `PermissionDeniedLayer` is wired into) for lack of permission, or because the
    /// organization isn't enrolled in client-credential support (spec FR73,
    /// `specs/rover-431-identity-grant-management`). `list`'s own FR16 branch does *not* use
    /// this: checking the Platform API's actual implementation showed its pairs-listing field
    /// returns the same empty, successful result for "no permission," "not enrolled," and
    /// "genuinely no pairs" alike (spec.md §6 has the full account) - there's nothing to
    /// classify there, so `list` only ever sees this kind of error from its own genuine-failure
    /// path ([`RoverClientError::PairListFailure`]). Distinct from
    /// [`RoverClientError::InvalidKey`] (an authentication failure) and from the generic
    /// [`RoverClientError::PermissionError`] (a different, graph-scoped permission story) -
    /// this one names the organization and points at the specific requirement.
    #[error(
        "You don't have permission to manage client-credential pairs in organization \
        `{organization_id}`. This requires the organization admin role, and during the initial \
        rollout the organization must be enrolled in client-credential support."
    )]
    PairPermissionDenied { organization_id: String },

    /// The Platform API refused an organization-scoped grant operation (revoking a user's
    /// grants) for lack of permission (spec FR73, `specs/rover-431-identity-grant-management`).
    /// FR73 requires one stable permission-denied code across the spec, so this shares
    /// [`RoverClientError::PairPermissionDenied`]'s code - it differs only in naming the action
    /// and the permission it needs.
    #[error(
        "You don't have permission to manage grants across organization `{organization_id}`. \
        This requires the organization's grant-management permission."
    )]
    GrantPermissionDenied { organization_id: String },

    /// `rover api-key rotate <ORGANIZATION_ID> <CLIENT_ID>` was given an ID that doesn't resolve
    /// to a client-credentials pair owned by that organization - genuinely nonexistent, belongs
    /// to a different organization, or isn't a `client_credentials` client at all (spec FR27,
    /// `specs/rover-431-identity-grant-management`). Distinct from
    /// [`RoverClientError::PairPermissionDenied`]: the rotate mutation checks org-level
    /// permission *before* looking at the client ID at all, so the two arrive as different
    /// failures from the Platform API, not two readings of the same ambiguous signal (unlike
    /// `list`'s FR16 case).
    #[error(
        "`{client_id}` isn't a client-credential pair in organization `{organization_id}`. \
        `rover api-key rotate` supports client-credential pairs only."
    )]
    PairNotFound {
        organization_id: String,
        client_id: String,
    },

    /// `rover api-key rotate --grace-period-days <DAYS>` was given a value too large to
    /// represent as a date (spec FR23 leaves the upper bound to the Platform API, but a date
    /// this far out overflows `chrono` itself before any request is ever made). Its own variant,
    /// not the generic [`RoverClientError::ClientError`], specifically so the command layer can
    /// tell it apart from a post-mutation failure: this one is raised *before* `rotate`'s mutation
    /// is ever sent, so - unlike every other error that reaches that match - nothing has
    /// changed, and "the outcome is unknown" would be actively misleading here.
    #[error("the requested grace period ({days} days) is too large to represent as a date")]
    GracePeriodTooLarge { days: i64 },

    /// `rover api-key rename <ORGANIZATION_ID> <ID> <NEW_NAME>` was given a client-credential
    /// pair's client ID. The Platform API has no way to rename a pair yet, so this fails without
    /// making any change (spec FR31, `specs/rover-431-identity-grant-management`).
    #[error(
        "`{client_id}` is a client-credential pair. Client-credential pairs can't be renamed."
    )]
    PairCannotBeRenamed { client_id: String },

    /// when attempting to create a key the associated Organization cannot be found
    #[error("Could not find the API Key with ID '{api_key_id}'")]
    ApiKeyNotFound { api_key_id: String },

    /// The user provided an invalid subgraph name.
    #[error("Could not find subgraph '{invalid_subgraph}'.")]
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
        "The graph registry does not contain variant '{}' for graph '{}'", graph_ref.variant(), graph_ref.graph_id()
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

    /// The request body was larger than GraphOS accepts (HTTP 413).
    #[error("The request was too large for GraphOS to process (HTTP 413 Payload Too Large).")]
    RequestTooLarge { endpoint_kind: EndpointKind },

    /// when someone provides a bad graph/variant combination or isn't
    /// validated properly, we don't know which reason is at fault for data.service
    /// being empty, so this error tells them to check both.
    #[error("Could not find graph with name '{graph_ref}'")]
    GraphNotFound { graph_ref: GraphRef },

    /// when someone provides a graph ID that doesn't exist.
    #[error("Could not find graph with ID '{graph_id}'")]
    GraphIdNotFound { graph_id: String },

    /// A publish reported that it triggered a launch, but that launch could
    /// not be found when polling for its status. This shouldn't be
    /// possible -- it likely indicates a race with Studio's own eventual
    /// consistency, or a bug in the publish/polling response handling.
    #[error("Could not find launch '{launch_id}' for '{graph_ref}'.")]
    LaunchNotFound {
        graph_ref: GraphRef,
        launch_id: String,
    },

    /// The requested graph artifact could not be found.
    #[error("Could not find the graph artifact: {msg}")]
    GraphArtifactNotFound { msg: String },

    /// The provided graph artifact digest was invalid.
    #[error("The graph artifact digest is invalid: {msg}")]
    GraphArtifactDigestInvalid { msg: String },

    /// The provided tag was invalid for the graph artifact.
    #[error("The graph artifact tag is invalid: {msg}")]
    GraphArtifactTagInvalid { msg: String },

    /// The tag could not be assigned to the graph artifact's variant.
    #[error("Could not assign the tag to the graph artifact variant: {msg}")]
    GraphArtifactTagVariantAssign { msg: String },

    /// The per-artifact tagging limit was exceeded.
    #[error("The graph artifact tagging limit was exceeded: {msg}")]
    GraphArtifactTaggingLimit { msg: String },

    /// The total tags limit for graph artifacts was exceeded.
    #[error("The total graph artifact tags limit was exceeded: {msg}")]
    GraphArtifactTotalTagsLimit { msg: String },

    /// An operation is already in progress on the graph artifact.
    #[error("An operation is already in progress: {msg}")]
    GraphArtifactOperationInProgress { msg: String },

    /// The graph artifact build failed, so it has no digest.
    #[error("The graph artifact build failed: {msg}")]
    GraphArtifactBuildFailed {
        msg: String,
        graph_id: String,
        launch_id: String,
    },

    /// if someone attempts to get a core schema from a supergraph that has
    /// no successful build in the API, we return this error.
    #[error("No supergraph SDL exists for '{graph_ref}' because its subgraphs failed to build.")]
    NoSupergraphBuilds {
        graph_ref: GraphRef,
        source: BuildErrors,
    },

    #[error("Encountered {} while trying to build a supergraph.", .source.length_string())]
    BuildErrors {
        source: BuildErrors,
        num_subgraphs: usize,
    },

    #[error("Encountered {} while trying to build subgraph '{subgraph}' into supergraph '{graph_ref}'.", .source.length_string())]
    SubgraphBuildErrors {
        subgraph: String,
        graph_ref: GraphRef,
        source: BuildErrors,
    },

    #[error("{}", contract_publish_errors_msg(.msgs, .no_launch))]
    ContractPublishErrors { msgs: Vec<String>, no_launch: bool },

    /// This error occurs when the Studio API returns no implementing services for a graph
    /// This response shouldn't be possible!
    #[error(
        "The response from Apollo Studio was malformed. Response body contains `null` value for '{null_field}'"
    )]
    MalformedResponse { null_field: String },

    /// This error occurs when an operation expected a federated graph but a non-federated
    /// graph was supplied.
    /// `can_operation_convert` is only set to true when a non-federated graph
    /// was encountered during an operation that could potentially convert a non-federated graph
    /// to a federated graph.
    #[error(
        "The graph `{graph_ref}` is a non-federated graph. This operation is only possible for federated graphs."
    )]
    ExpectedFederatedGraph {
        graph_ref: GraphRef,
        can_operation_convert: bool,
    },

    /// This error occurs when an operation expected a contract variant but a non-contract variant
    /// was supplied.
    #[error(
        "The variant `{graph_ref}` is a non-contract variant. This operation is only possible for contract variants."
    )]
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

    /// `graph publish --check`/`subgraph publish --check` didn't publish,
    /// because the check failed. Carries the check result, like
    /// [`RoverClientError::CheckWorkflowFailure`], so `--format json` reports
    /// it as `data`; the publish commands print the check's text report
    /// themselves, before this error.
    #[error(
        "Schema checks must pass before publishing. Fix the check failures above and try again."
    )]
    PublishCheckFailure {
        graph_ref: GraphRef,
        check_response: Box<CheckWorkflowResponse>,
    },

    /// This error occurs when `graph publish`/`subgraph publish` succeeded,
    /// but a launch it triggered (or one of the downstream contract-variant
    /// launches it triggered) did not complete successfully. `publish_response`
    /// is pre-serialized rather than the concrete `GraphPublishResponse`/
    /// `SubgraphPublishResponse` type, since this crate's error type has no
    /// other reason to depend on either operation's response type directly.
    #[error(
        "The publish to '{graph_ref}' succeeded, but a triggered launch did not complete successfully. See the launch report above for details."
    )]
    PublishLaunchFailure {
        graph_ref: GraphRef,
        publish_response: serde_json::Value,
    },

    /// `rover api-key list`'s client-credential pairs query genuinely failed (a timeout, a 5xx,
    /// any real error - an empty, successful pairs result is not this error; see spec FR16,
    /// `specs/rover-431-identity-grant-management`). `keys` is the best-effort API-key list
    /// already fetched, carried so the command still reports it despite the overall failure -
    /// mirrors [`RoverClientError::PublishLaunchFailure`]/
    /// [`RoverClientError::CheckWorkflowFailure`]'s existing pattern of a variant carrying
    /// already-fetched structured data for the binary crate's `RoverError::print()`/
    /// `get_internal_data_json()` to render at print time. `keys` is `None` when API keys were
    /// never in scope either (spec FR17: `--type` named `client-credentials` alone) - there is
    /// nothing left to show best-effort, so the command fails outright with no output.
    #[error("{}", pair_list_failure_message(organization_id, &**source, keys))]
    PairListFailure {
        organization_id: String,
        keys: Option<Vec<ApiKey>>,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// While linting the proposed schema, some rule violations were found
    #[error("While linting the proposed schema, some rule violations were found")]
    LintFailures { lint_response: LintResponse },

    /// Encountered errors while converting a persisted query manifest generated by the Relay compiler to the structure
    /// required by Apollo GraphOS
    #[error(
        "The persisted query manifest generated by the Relay compiler contained the following errors:\n\n{errors}"
    )]
    RelayOperationParseFailures { errors: String },

    /// This error occurs when a user has a malformed Graph Ref
    #[error(transparent)]
    InvalidGraphRef(#[from] InvalidGraphRef),

    /// This error occurs when a user has a malformed API key
    #[error(
        "The API key you provided is malformed. An API key must have three non-empty parts separated by colons."
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

    #[error("Failed to create graph after {max_retries} retries. Please try again.")]
    MaxRetriesExceeded { max_retries: u8 },

    #[error(
        "You cannot perform this operation due to a limit imposed by your current billing plan"
    )]
    PlanError { msg: String },

    #[error("The check workflow took too long to run.")]
    ChecksTimeoutError { url: Option<String> },

    #[error("The schema check finished, but Rover could not retrieve its full result.")]
    CheckWorkflowResultUnavailable {
        url: Option<String>,
        #[source]
        source: Box<RoverClientError>,
    },

    #[error(
        "Timed out waiting for preview {build_id} to complete. Check back with `--build-id {build_id}`, or raise APOLLO_CHECKS_TIMEOUT_SECONDS."
    )]
    PreviewTimeoutError { build_id: String },

    #[error("Preview {build_id} finished, but Rover could not retrieve its result.")]
    PreviewResultUnavailable {
        build_id: String,
        #[source]
        source: Box<RoverClientError>,
    },

    #[error("Timed out waiting for the launch to complete.")]
    LaunchTimeoutError { url: Option<String> },

    /// `graph publish`/`subgraph publish` stopped waiting for the launch it
    /// triggered. Like [`RoverClientError::PublishLaunchFailure`], the publish
    /// itself already succeeded, so this carries the publish response
    /// (pre-serialized, for the same reason) for `--format json` to report as
    /// `data`, instead of discarding it. `launch_status` in it is `null`:
    /// Rover stopped waiting before it learned the outcome.
    #[error("Timed out waiting for the launch to complete.")]
    PublishLaunchTimeout {
        url: Option<String>,
        publish_response: serde_json::Value,
    },

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

    #[error(
        "Could not find a persisted query list with ID '{list_id}' associated with the '{graph_id}' graph."
    )]
    PersistedQueryListIdNotFound {
        graph_id: String,
        list_id: String,
        frontend_url_root: String,
    },

    #[error("Offline licences are not enabled for your organization.")]
    OfflineLicenseNotEnabled,

    #[error("You've encountered a rate limit.")]
    RateLimitExceeded,

    #[error("Something went wrong on our end. This isn't your fault! Please try again.")]
    GraphProjectInitError,

    #[error("Service failed to become ready")]
    ServiceReady(Box<dyn std::error::Error + Send + Sync>),

    #[error("Service error")]
    Service {
        source: Box<dyn std::error::Error + Send + Sync>,
        endpoint_kind: EndpointKind,
    },

    /// Failed to parse Graph creation response coming from server.
    #[error("{msg}")]
    GraphCreationError {
        /// Graph creation error coming from schema encoder.
        msg: String,
    },
}

impl RoverClientError {
    pub(crate) const fn is_transient(&self) -> bool {
        matches!(
            self,
            RoverClientError::SendRequest { .. } | RoverClientError::RateLimitExceeded
        )
    }
}

/// The trailing sentence on [`RoverClientError::PairListFailure`]'s message - present only when
/// `keys` actually has something to point at (spec FR16); a pairs-only failure (FR17, `keys:
/// None`) reports no keys at all, so claiming keys are shown would be false.
const fn pair_list_failure_suffix(keys: &Option<Vec<ApiKey>>) -> &'static str {
    if keys.is_some() {
        " API keys are shown above."
    } else {
        ""
    }
}

/// [`RoverClientError::PairListFailure`]'s full message. `source`'s own rendered message is the
/// Platform API's, verbatim and out of Rover's control - it may or may not already end in a
/// period, so trim one off before appending Rover's own sentence(s) rather than risk a doubled
/// `"..timed out.. API keys are shown above."`.
fn pair_list_failure_message(
    organization_id: &str,
    source: &(dyn std::error::Error + Send + Sync),
    keys: &Option<Vec<ApiKey>>,
) -> String {
    let source_message = source.to_string();
    let source_message = source_message.trim_end_matches('.');
    format!(
        "Rover couldn't list client-credential pairs in organization `{organization_id}`: {source_message}.{}",
        pair_list_failure_suffix(keys)
    )
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
        [single_task] => {
            format!("The changes in the schema you proposed caused {single_task} checks to fail.")
        }
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

impl RoverClientError {
    /// A failure from a Studio GraphQL service, reported as a credential problem when that is
    /// what it turned out to be and otherwise carrying the original error and its endpoint.
    ///
    /// Operations that need their own arms for some variants should reach for this in place of
    /// building [`RoverClientError::Service`] directly, so a rejected credential doesn't get
    /// flattened into an error that carries no code.
    pub fn studio_service<T: Debug + Send + Sync + 'static>(
        err: GraphQLServiceError<T>,
    ) -> RoverClientError {
        rejected_credential_in(&err).unwrap_or(RoverClientError::Service {
            source: Box::new(err),
            endpoint_kind: EndpointKind::ApolloStudio,
        })
    }
}

/// How this failure should be reported if the registry refused the credential, covering both
/// ways it can say so: a rejection by status, spotted by `RejectedCredentialLayer`, and one in
/// the response body.
///
/// Only the layer can tell a malformed key from a merely unrecognized one, because only the
/// layer holds the credential. A body-level rejection can only ever be the weaker answer.
fn rejected_credential_in<T>(err: &GraphQLServiceError<T>) -> Option<RoverClientError>
where
    T: Debug + Send + Sync,
{
    match err {
        GraphQLServiceError::UpstreamService(source) => {
            match source
                .downcast_ref::<HttpServiceError>()
                .and_then(rejected_credential)?
            {
                RejectedCredential::MalformedKey => Some(RoverClientError::MalformedKey),
                RejectedCredential::InvalidKey => Some(RoverClientError::InvalidKey),
            }
        }
        GraphQLServiceError::InvalidCredentials() => Some(RoverClientError::InvalidKey),
        _ => None,
    }
}

/// Recovers a client-credential-pair permission denial from a raw GraphQL error, for an
/// operation whose mutation returns a plain object rather than a typed error union (see
/// `rover_studio::service::permission_denied`'s own doc comment: for these, a permission denial
/// can only ever arrive as a raw HTTP 403). Unlike [`rejected_credential_in`], this isn't wired
/// into the blanket [`From<GraphQLServiceError<T>>`] conversion below - it takes
/// `organization_id`, which that conversion has no way to know, so each pair operation's own
/// `service.rs` calls this directly on its inner call's error before falling back to `.into()`.
pub(crate) fn permission_denied_in<T>(
    err: &GraphQLServiceError<T>,
    organization_id: impl Into<String>,
) -> Option<RoverClientError>
where
    T: Debug + Send + Sync,
{
    is_permission_denied(err).then(|| RoverClientError::PairPermissionDenied {
        organization_id: organization_id.into(),
    })
}

/// [`permission_denied_in`]'s counterpart for organization-scoped grant operations: the same
/// raw-403 recovery, reported with FR73's grants wording instead of the pairs one.
pub(crate) fn grant_permission_denied_in<T>(
    err: &GraphQLServiceError<T>,
    organization_id: impl Into<String>,
) -> Option<RoverClientError>
where
    T: Debug + Send + Sync,
{
    is_permission_denied(err).then(|| RoverClientError::GrantPermissionDenied {
        organization_id: organization_id.into(),
    })
}

fn is_permission_denied<T>(err: &GraphQLServiceError<T>) -> bool
where
    T: Debug + Send + Sync,
{
    match err {
        GraphQLServiceError::UpstreamService(source) => source
            .downcast_ref::<HttpServiceError>()
            .and_then(permission_denied)
            .is_some(),
        _ => false,
    }
}

/// The exact substring the Platform API's `OAuthClientError.ClientNotFound` renders as (confirmed
/// by reading `apps/identity/service/services/.../OAuth2ManagementService.kt`'s
/// `validateAdminTarget`/`OAuthClientError.kt` in the Platform API's own source - not a
/// contractually-guaranteed wire format, just the message text as of this writing). Covers a
/// nonexistent client ID, one belonging to another organization, and one that isn't a
/// `client_credentials` client at all - the backend deliberately reports all three identically
/// (the same "don't reveal what you can't manage" shape as `list`'s FR16 ambiguity), so Rover
/// can't and doesn't try to tell them apart either.
const PAIR_NOT_FOUND_MESSAGE: &str = "Client not found for client";

/// Recovers a "this ID isn't a client-credential pair" failure (spec FR27) from a raw GraphQL
/// error, by matching [`PAIR_NOT_FOUND_MESSAGE`] against each error's own message - the only
/// signal available, since `rotateOAuthClientSecret` returns a plain object with no typed error
/// union (same reasoning as [`permission_denied_in`]'s doc comment, for the "not found" case
/// instead of the permission-denied one). A reviewer should treat this classification as resting
/// on confirmed-via-source, not contractually-guaranteed, backend behavior.
pub(crate) fn pair_not_found_in<T>(
    err: &GraphQLServiceError<T>,
    organization_id: impl Into<String>,
    client_id: impl Into<String>,
) -> Option<RoverClientError>
where
    T: Debug + Send + Sync,
{
    let errors = match err {
        GraphQLServiceError::NoData(errors) => errors,
        GraphQLServiceError::PartialError { errors, .. } => errors,
        _ => return None,
    };
    errors
        .iter()
        .any(|error| error.message.contains(PAIR_NOT_FOUND_MESSAGE))
        .then(|| RoverClientError::PairNotFound {
            organization_id: organization_id.into(),
            client_id: client_id.into(),
        })
}

impl<T: Debug + Send + Sync> From<GraphQLServiceError<T>> for RoverClientError {
    fn from(value: GraphQLServiceError<T>) -> Self {
        if let Some(rejection) = rejected_credential_in(&value) {
            return rejection;
        }
        match value {
            GraphQLServiceError::NoData(errors) => RoverClientError::GraphQl {
                msg: if errors.is_empty() {
                    "No data field provided".to_string()
                } else {
                    errors.iter().map(|err| err.to_string()).join("\n")
                },
            },
            GraphQLServiceError::PartialError { errors, .. } => {
                let errors = errors.iter().map(|err| err.to_string()).join("\n");
                RoverClientError::GraphQl {
                    msg: format!("Response returned with errors:\n{errors}"),
                }
            }
            // `ClientError` carries only a message, so the chain stops here — render it in full
            // rather than dropping every cause below the outermost one.
            _ => RoverClientError::ClientError {
                msg: rover_std::format_error_chain(&value),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use rover_studio::service::rejected_credential::RejectedCredential;
    use speculoos::prelude::*;

    use super::*;

    /// How a rejection reaches this conversion: wrapped in `HttpServiceError` to cross the boxed
    /// `HttpService`, then in `UpstreamService` by the GraphQL middleware above it.
    fn as_upstream_error(rejection: RejectedCredential) -> GraphQLServiceError<()> {
        GraphQLServiceError::UpstreamService(Box::new(HttpServiceError::Unexpected(Box::new(
            rejection,
        ))))
    }

    #[test]
    fn a_rejected_malformed_key_surfaces_as_malformed_key() {
        let err = RoverClientError::from(as_upstream_error(RejectedCredential::MalformedKey));

        assert!(matches!(err, RoverClientError::MalformedKey));
    }

    #[test]
    fn a_rejected_well_formed_key_surfaces_as_invalid_key() {
        let err = RoverClientError::from(as_upstream_error(RejectedCredential::InvalidKey));

        assert!(matches!(err, RoverClientError::InvalidKey));
    }

    #[test]
    fn body_level_invalid_credentials_surface_as_invalid_key() {
        let err = RoverClientError::from(GraphQLServiceError::<()>::InvalidCredentials());

        assert!(matches!(err, RoverClientError::InvalidKey));
    }

    // Operations with their own error arms use this instead of building `Service` directly, so a
    // rejected credential has to survive that path too.
    #[test]
    fn studio_service_reports_a_rejected_credential_as_a_key_error() {
        let malformed =
            RoverClientError::studio_service(as_upstream_error(RejectedCredential::MalformedKey));
        let invalid =
            RoverClientError::studio_service(as_upstream_error(RejectedCredential::InvalidKey));
        let body_level =
            RoverClientError::studio_service(GraphQLServiceError::<()>::InvalidCredentials());

        assert!(matches!(malformed, RoverClientError::MalformedKey));
        assert!(matches!(invalid, RoverClientError::InvalidKey));
        assert!(matches!(body_level, RoverClientError::InvalidKey));
    }

    #[test]
    fn service_message_excludes_its_cause() {
        let err = RoverClientError::Service {
            source: Box::<dyn std::error::Error + Send + Sync>::from(
                "the upstream service refused the request",
            ),
            endpoint_kind: EndpointKind::ApolloStudio,
        };

        assert_that!(err.to_string()).is_equal_to("Service error".to_string());
        assert_that!(rover_std::format_error_chain(&err))
            .is_equal_to("Service error: the upstream service refused the request".to_string());
    }

    /// `ClientError` carries only a message, so the conversion has to render the whole chain
    /// into it — there is no `source` left for anything downstream to walk.
    #[test]
    fn converting_an_upstream_service_error_keeps_the_whole_chain_in_the_message() {
        let err = RoverClientError::from(GraphQLServiceError::<()>::UpstreamService(Box::new(
            HttpServiceError::TimedOut,
        )));

        assert_that!(err.to_string()).is_equal_to(
            "Unable to get a response from an endpoint. Client returned an error.\n\nUpstream service error: Request timed out"
                .to_string(),
        );
    }

    // Everything else keeps the wrapper the operations already relied on, so this doesn't
    // reclassify unrelated failures.
    #[test]
    fn studio_service_keeps_other_failures_as_service_errors() {
        let err = RoverClientError::studio_service(GraphQLServiceError::<()>::UpstreamService(
            Box::new(HttpServiceError::TimedOut),
        ));

        assert!(matches!(
            err,
            RoverClientError::Service {
                endpoint_kind: EndpointKind::ApolloStudio,
                ..
            }
        ));
    }

    #[test]
    fn other_upstream_errors_still_surface_as_client_errors() {
        let err = RoverClientError::from(GraphQLServiceError::<()>::UpstreamService(Box::new(
            HttpServiceError::TimedOut,
        )));

        assert!(matches!(err, RoverClientError::ClientError { .. }));
    }

    mod pair_list_failure_message {
        use super::*;

        /// A bare error whose `Display` is exactly its message - unlike
        /// `RoverClientError::ClientError`, which decorates `msg` with its own extra text, this
        /// lets these tests assert on the composed message's exact literal wording.
        #[derive(Debug)]
        struct BareError(&'static str);

        impl std::fmt::Display for BareError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::error::Error for BareError {}

        fn source(msg: &'static str) -> BareError {
            BareError(msg)
        }

        // A source message that doesn't already end in a period gets exactly one.
        #[test]
        fn appends_a_single_period_when_the_source_has_none() {
            let message =
                pair_list_failure_message("acme", &source("timed out"), &Some(Vec::new()));

            assert_that!(message).is_equal_to(
                "Rover couldn't list client-credential pairs in organization `acme`: timed out. \
                 API keys are shown above."
                    .to_string(),
            );
        }

        // A source message that already ends in a period doesn't get a second one - the
        // Platform API's own message text is out of Rover's control.
        #[test]
        fn does_not_double_a_period_the_source_already_has() {
            let message =
                pair_list_failure_message("acme", &source("timed out."), &Some(Vec::new()));

            assert_that!(message).is_equal_to(
                "Rover couldn't list client-credential pairs in organization `acme`: timed out. \
                 API keys are shown above."
                    .to_string(),
            );
        }

        // FR17: no keys in scope - the trailing "API keys are shown above" sentence is dropped
        // entirely rather than claim something false.
        #[test]
        fn drops_the_keys_sentence_when_keys_is_none() {
            let message = pair_list_failure_message("acme", &source("timed out"), &None);

            assert_that!(message).is_equal_to(
                "Rover couldn't list client-credential pairs in organization `acme`: timed out."
                    .to_string(),
            );
        }
    }
}
