use thiserror::Error;

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
    /*
    /// Encountered an error handling the received response.
    #[error("{msg}")]
    AdhocError {
        /// The error message.
        msg: String,
    },*/
    /// Encountered a 400-599 error from an endpoint.
    #[error("Unable to get a response from an endpoint. Client returned an error.\n\n{msg}")]
    ClientError {
        /// Error message from client.
        msg: String,
    },
    /*
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
    */
    /// Encountered an error sending the request.
    #[error(transparent)]
    SendRequest(#[from] reqwest::Error),

    /// This error occurs when the Studio API returns no implementing services for a graph
    /// This response shouldn't be possible!
    #[error("The response from Apollo Studio was malformed. Response body contains `null` value for \"{null_field}\"")]
    MalformedResponse { null_field: String },
    /*
        /// The API returned an invalid ChangeSeverity value
        #[error("Invalid ChangeSeverity.")]
        InvalidSeverity,

        /// The user supplied an invalid validation period
        #[error("You can only specify a duration as granular as seconds.")]
        ValidationPeriodTooGranular,

        /// The user supplied an invalid validation period duration
        #[error(transparent)]
        InvalidValidationPeriodDuration(#[from] humantime::DurationError),

        /// This error occurs when a user has a malformed Graph Ref
        #[error("Graph IDs must be in the format <NAME> or <NAME>@<VARIANT>, where <NAME> can only contain letters, numbers, or the characters `-` or `_`, and must be 64 characters or less. <VARIANT> must be 64 characters or less.")]
        InvalidGraphRef,
    */
    /// This error occurs when a user has a malformed API key
    #[error(
        "The API key you provided is malformed. An API key must have three parts separated by a colon."
    )]
    MalformedKey,
    /*
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
    SubgraphIntrospectionNotAvailable,*/
}

/*
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
*/
