/// The possible error types that can occur in Rover.
#[derive(Debug)]
pub enum RoverErrorKind {
    /// An error returned by Apollo Studio
    StudioError(StudioErrorKind),

    /// An error that occurred in a different library
    ExternalError(ExternalErrorKind),

    /// This error occurs when attempting to execute an operation intended
    /// for federated graphs on a non-federated graph.
    ExpectedFederatedGraph { graph_name: String },
}

impl std::fmt::Display for RoverErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let message = match self {
            RoverErrorKind::StudioError(kind) => match kind {
                _ => format!("{:?}", kind),
            },
            RoverErrorKind::ExpectedFederatedGraph { graph_name } => format!(
                "Could not execute this operation on {} because it is not a federated graph.",
                graph_name
            ),
            RoverErrorKind::ExternalError(_) => {
                panic!("External errors should not be displayed via this Display implementation")
            }
        };
        write!(f, "{}", &message)
    }
}

#[derive(Debug)]
pub enum StudioErrorKind {
    /// This error occurs when the provided API Token is invalid.
    AccessForbidden,

    /// This error occurs when the API returns no composition errors AND
    /// no check result.
    NoCheckData,

    /// This error occurs when there are no `body.errors` but `body.data` is
    /// also empty. In proper GraphQL responses, there should _always_ be either
    /// body.errors or body.data
    NoData,

    /// This error occurs when the API returns an invalid ChangeSeverity value
    InvalidChangeSeverity,

    /// This error occurs when an invalid service/variant combo is provided
    InvalidService,

    /// This is a passthrough for errors returned by the Studio API
    Other { msg: String },
}

#[derive(Debug)]
pub enum ExternalErrorKind {
    /// This error occurs when something goes wrong while
    /// performing an HTTP request
    Request,

    /// This error occurs when an HTTP response returns invalid JSON
    InvalidJSON,

    /// This error occurs when attempting to build a HeaderMap with an
    /// invalid name
    InvalidHeaderName,

    /// This error occurs when attempting to build a HeaderMap with an
    /// invalid value
    InvalidHeaderValue,
}
