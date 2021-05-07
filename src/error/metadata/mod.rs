mod code;
mod suggestion;

pub(crate) use code::Code;
pub use suggestion::Suggestion;

use houston::HoustonProblem;
use rover_client::RoverClientError;

use crate::utils::env::RoverEnvKey;

use std::{env, fmt::Display};

use ansi_term::Colour::Red;

/// Metadata contains extra information about specific errors
/// Currently this includes an optional error `Code`
/// and an optional `Suggestion`
#[derive(Default, Debug)]
pub struct Metadata {
    pub suggestion: Option<Suggestion>,
    pub code: Option<Code>,
    pub is_parse_error: bool,
}

impl Metadata {
    pub fn parse_error(suggestion: impl Display) -> Self {
        Metadata {
            suggestion: Some(Suggestion::Adhoc(suggestion.to_string())),
            code: None,
            is_parse_error: true,
        }
    }
}

/// `Metadata` structs can be created from an `anyhow::Error`
/// This works by downcasting the errors to their underlying types
/// and creating `Suggestion`s and `Code`s where applicable
impl From<&mut anyhow::Error> for Metadata {
    fn from(error: &mut anyhow::Error) -> Self {
        if let Some(rover_client_error) = error.downcast_ref::<RoverClientError>() {
            let (suggestion, code) = match rover_client_error {
                RoverClientError::InvalidJson(_)
                | RoverClientError::InvalidHeaderName(_)
                | RoverClientError::InvalidHeaderValue(_)
                | RoverClientError::MalformedResponse { null_field: _ }
                | RoverClientError::InvalidSeverity => (Some(Suggestion::SubmitIssue), None),
                RoverClientError::SendRequest(_) => (None, None),
                RoverClientError::CouldNotConnect { .. } => {
                    (Some(Suggestion::CheckServerConnection), None)
                }
                RoverClientError::NoCompositionPublishes {
                    graph: _,
                    composition_errors,
                } => {
                    for composition_error in composition_errors {
                        eprintln!("{} {}", Red.bold().paint("error:"), composition_error);
                    }
                    (Some(Suggestion::RunComposition), None)
                }
                RoverClientError::ExpectedFederatedGraph { graph: _ } => {
                    (Some(Suggestion::UseFederatedGraph), None)
                }
                RoverClientError::NoSchemaForVariant {
                    graph,
                    invalid_variant,
                    valid_variants,
                    frontend_url_root,
                } => (
                    Some(Suggestion::ProvideValidVariant {
                        graph_name: graph.clone(),
                        invalid_variant: invalid_variant.clone(),
                        valid_variants: valid_variants.clone(),
                        frontend_url_root: frontend_url_root.clone(),
                    }),
                    None,
                ),
                RoverClientError::NoSubgraphInGraph {
                    invalid_subgraph: _,
                    valid_subgraphs,
                } => (
                    Some(Suggestion::ProvideValidSubgraph(valid_subgraphs.clone())),
                    None,
                ),
                RoverClientError::NoService { graph: _ } => {
                    (Some(Suggestion::CheckGraphNameAndAuth), None)
                }
                RoverClientError::AdhocError { msg: _ }
                | RoverClientError::GraphQl { msg: _ }
                | RoverClientError::IntrospectionError { msg: _ }
                | RoverClientError::ClientError { msg: _ } => (None, None),
                RoverClientError::InvalidKey => (Some(Suggestion::CheckKey), None),
                RoverClientError::MalformedKey => (Some(Suggestion::ProperKey), None),
                RoverClientError::UnparseableReleaseVersion => {
                    (Some(Suggestion::SubmitIssue), None)
                }
            };
            return Metadata {
                suggestion,
                code,
                is_parse_error: false,
            };
        }

        if let Some(houston_problem) = error.downcast_ref::<HoustonProblem>() {
            let (suggestion, code) = match houston_problem {
                HoustonProblem::CouldNotCreateConfigHome(_)
                | HoustonProblem::DefaultConfigDirNotFound
                | HoustonProblem::InvalidOverrideConfigDir(_) => {
                    (Some(Suggestion::SetConfigHome), None)
                }
                HoustonProblem::NoConfigFound(_) => {
                    let code = None;
                    let suggestion = if env::var_os(RoverEnvKey::ConfigHome.to_string()).is_some() {
                        Some(Suggestion::MigrateConfigHomeOrCreateConfig)
                    } else {
                        Some(Suggestion::CreateConfig)
                    };
                    (suggestion, code)
                }
                HoustonProblem::NoConfigProfiles => (Some(Suggestion::NewUserNoProfiles), None),
                HoustonProblem::ProfileNotFound(_) => (Some(Suggestion::ListProfiles), None),
                HoustonProblem::NoNonSensitiveConfigFound(_)
                | HoustonProblem::PathNotUtf8(_)
                | HoustonProblem::TomlDeserialization(_)
                | HoustonProblem::TomlSerialization(_)
                | HoustonProblem::IoError(_) => (Some(Suggestion::SubmitIssue), None),
            };
            return Metadata {
                suggestion,
                code,
                is_parse_error: false,
            };
        }

        Metadata::default()
    }
}
