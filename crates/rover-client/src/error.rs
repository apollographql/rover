use thiserror::Error;

use crate::shared::{CheckResponse, GraphRef};

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
    #[error(transparent)]
    SendRequest(#[from] reqwest::Error),

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
    BuildErrors { source: BuildErrors },

    #[error("Encountered {} while trying to build subgraph \"{subgraph}\" into supergraph \"{graph_ref}\".", .source.length_string())]
    SubgraphBuildErrors {
        subgraph: String,
        graph_ref: GraphRef,
        source: BuildErrors,
    },

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
        check_response: CheckResponse,
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
    InvalidInputError {
        msg: String,
    },

    #[error("You don't have the required permissions to perform this operation")]
    PermissionError{
        msg: String,
    },

    #[error("You cannot perform this operation due to a limit imposed by your current billing plan")]
    PlanError{
        msg: String,
    },
}

fn operation_check_error_msg(check_response: &CheckResponse) -> String {
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

impl From<launchpad::error::RoverClientError> for RoverClientError {
    fn from(e: launchpad::error::RoverClientError) -> Self {
        match e {
            launchpad::error::RoverClientError::GraphQl { msg } => {
                RoverClientError::GraphQl { msg }
            }
            launchpad::error::RoverClientError::IntrospectionError { msg } => {
                RoverClientError::IntrospectionError { msg }
            }
            launchpad::error::RoverClientError::InvalidHeaderName(h) => {
                RoverClientError::InvalidHeaderName(h)
            }
            launchpad::error::RoverClientError::InvalidHeaderValue(v) => {
                RoverClientError::InvalidHeaderValue(v)
            }
            launchpad::error::RoverClientError::InvalidJson(j) => RoverClientError::InvalidJson(j),
            launchpad::error::RoverClientError::ClientError { msg } => {
                RoverClientError::ClientError { msg }
            }
            launchpad::error::RoverClientError::SendRequest(req) => {
                RoverClientError::SendRequest(req)
            }
            launchpad::error::RoverClientError::MalformedResponse { null_field } => {
                RoverClientError::MalformedResponse { null_field }
            }
            launchpad::error::RoverClientError::MalformedKey => RoverClientError::MalformedKey,
        }
    }
}
