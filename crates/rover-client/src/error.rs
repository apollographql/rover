use thiserror::Error;

use crate::shared::{GraphRef, OperationCheckResponse};

use apollo_federation_types::build::BuildErrors;

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

    /// Tried to build a [HeaderMap] with an invalid header name.
    #[error("Invalid header name")]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),

    /// Tried to build a [HeaderMap] with an invalid header value.
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
        is_studio: bool,
    },

    /// when someone provides a bad graph/variant combination or isn't
    /// validated properly, we don't know which reason is at fault for data.service
    /// being empty, so this error tells them to check both.
    #[error("Could not find graph with name \"{graph_ref}\"")]
    GraphNotFound { graph_ref: GraphRef },

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

    /// While checking the proposed schema, we encountered changes that would break existing operations
    // we nest the CheckResponse here because we want to print the entire response even
    // if there were failures
    #[error("{}", operation_check_error_msg(.check_response))]
    OperationCheckFailure {
        graph_ref: GraphRef,
        check_response: OperationCheckResponse,
    },

    /// While checking the proposed schema, we encountered changes that would cause checks to fail in
    /// blocking downstream variants.
    #[error("{}", downstream_check_error_msg(.blocking_downstream_variants))]
    DownstreamCheckFailure {
        blocking_downstream_variants: Vec<String>,
        target_url: String,
    },

    /// While checking the proposed schema, the build, operations, and downstream (if run) tasks succeeded
    /// or are pending, but other check tasks failed.
    #[error("{}", other_check_task_failure_msg(.has_build_task,.has_downstream_task))]
    OtherCheckTaskFailure {
        has_build_task: bool,
        has_downstream_task: bool,
        target_url: String,
    },

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

    #[error("You don't have the required permissions to perform this operation")]
    PermissionError { msg: String },

    #[error(
        "You cannot perform this operation due to a limit imposed by your current billing plan"
    )]
    PlanError { msg: String },

    #[error("Your check took too long to run")]
    ChecksTimeoutError { url: Option<String> },

    #[error("You cannot publish a new subgraph without specifying a routing URL.")]
    MissingRoutingUrlError {
        subgraph_name: String,
        graph_ref: GraphRef,
    },
}

fn contract_publish_errors_msg(msgs: &Vec<String>, no_launch: &bool) -> String {
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

fn operation_check_error_msg(check_response: &OperationCheckResponse) -> String {
    let failure_count = check_response.get_failure_count();
    let plural = match failure_count {
        1 => "",
        _ => "s",
    };
    format!(
        "This operation check has encountered {} schema change{} that would break operations from existing client traffic.",
        failure_count, plural
    )
}

fn downstream_check_error_msg(downstream_blocking_variants: &Vec<String>) -> String {
    let variants = downstream_blocking_variants.join(",");
    let plural_this = match downstream_blocking_variants.len() {
        1 => "this",
        _ => "these",
    };
    let plural = match downstream_blocking_variants.len() {
        1 => "",
        _ => "s",
    };
    format!(
        "The downstream check task has encountered check failures for at least {} blocking downstream variant{}: {}.",
        plural_this,
        plural,
        variants,
    )
}

fn other_check_task_failure_msg(has_build_task: &bool, has_downstream_task: &bool) -> String {
    let succeeding_tasks = match (*has_build_task, *has_downstream_task) {
        (false, false) => "The operations task",
        (true, false) => "The build and operations tasks",
        (true, true) => "The build, operations, and downstream tasks",
        (false, true) => unreachable!("Can't have a downstream task without a build task"),
    };
    format!(
        "{} succeeded or are pending, but other check tasks failed.",
        succeeding_tasks
    )
}

impl From<introspector_gadget::error::RoverClientError> for RoverClientError {
    fn from(e: introspector_gadget::error::RoverClientError) -> Self {
        match e {
            introspector_gadget::error::RoverClientError::GraphQl { msg } => {
                RoverClientError::GraphQl { msg }
            }
            introspector_gadget::error::RoverClientError::IntrospectionError { msg } => {
                RoverClientError::IntrospectionError { msg }
            }
            introspector_gadget::error::RoverClientError::InvalidHeaderName(h) => {
                RoverClientError::InvalidHeaderName(h)
            }
            introspector_gadget::error::RoverClientError::InvalidHeaderValue(v) => {
                RoverClientError::InvalidHeaderValue(v)
            }
            introspector_gadget::error::RoverClientError::InvalidJson(j) => {
                RoverClientError::InvalidJson(j)
            }
            introspector_gadget::error::RoverClientError::ClientError { msg } => {
                RoverClientError::ClientError { msg }
            }
            introspector_gadget::error::RoverClientError::SendRequest(req) => {
                RoverClientError::SendRequest {
                    source: req,
                    is_studio: false,
                }
            }
            introspector_gadget::error::RoverClientError::MalformedResponse { null_field } => {
                RoverClientError::MalformedResponse { null_field }
            }
            introspector_gadget::error::RoverClientError::MalformedKey => {
                RoverClientError::MalformedKey
            }
        }
    }
}
