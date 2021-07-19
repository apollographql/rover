pub(crate) mod code;
mod suggestion;

pub(crate) use code::Code;
pub use suggestion::Suggestion;

use houston::HoustonProblem;
use rover_client::RoverClientError;

use crate::utils::env::RoverEnvKey;

use std::{env, fmt::Display};

use serde::Serialize;

/// Metadata contains extra information about specific errors
/// Currently this includes an optional error `Code`
/// and an optional `Suggestion`
#[derive(Default, Serialize, Debug)]
pub struct Metadata {
    // skip serializing for now until we can appropriately strip color codes
    #[serde(skip_serializing)]
    pub suggestion: Option<Suggestion>,
    pub code: Option<Code>,

    #[serde(skip_serializing)]
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
                RoverClientError::InvalidJson(_) => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E001))
                }
                RoverClientError::InvalidHeaderName(_) => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E002))
                }
                RoverClientError::InvalidHeaderValue(_) => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E003))
                }
                RoverClientError::SendRequest(_) => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E004))
                }
                RoverClientError::MalformedResponse { null_field: _ } => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E005))
                }
                RoverClientError::InvalidSeverity => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E006))
                }
                RoverClientError::SubgraphCompositionErrors {
                    graph_ref,
                    subgraph,
                    source: _,
                } => (
                    Some(Suggestion::FixSubgraphSchema {
                        graph_ref: graph_ref.clone(),
                        subgraph: subgraph.clone(),
                    }),
                    Some(Code::E029),
                ),
                RoverClientError::CompositionErrors { source: _ } => {
                    (Some(Suggestion::FixCompositionErrors), Some(Code::E029))
                }
                RoverClientError::OperationCheckFailure {
                    graph_ref,
                    check_response: _,
                } => (
                    Some(Suggestion::FixOperationsInSchema {
                        graph_ref: graph_ref.clone(),
                    }),
                    Some(Code::E030),
                ),
                RoverClientError::SubgraphIntrospectionNotAvailable => {
                    (Some(Suggestion::UseFederatedGraph), Some(Code::E007))
                }
                RoverClientError::ExpectedFederatedGraph {
                    graph_ref: _,
                    can_operation_convert,
                } => {
                    if *can_operation_convert {
                        (Some(Suggestion::ConvertGraphToSubgraph), Some(Code::E007))
                    } else {
                        (Some(Suggestion::UseFederatedGraph), Some(Code::E007))
                    }
                }
                RoverClientError::NoSchemaForVariant {
                    graph_ref,
                    valid_variants,
                    frontend_url_root,
                } => (
                    Some(Suggestion::ProvideValidVariant {
                        graph_ref: graph_ref.clone(),
                        valid_variants: valid_variants.clone(),
                        frontend_url_root: frontend_url_root.clone(),
                    }),
                    Some(Code::E008),
                ),
                RoverClientError::NoSubgraphInGraph {
                    invalid_subgraph: _,
                    valid_subgraphs,
                } => (
                    Some(Suggestion::ProvideValidSubgraph(valid_subgraphs.clone())),
                    Some(Code::E009),
                ),
                RoverClientError::GraphNotFound { .. } => {
                    (Some(Suggestion::CheckGraphNameAndAuth), Some(Code::E010))
                }
                RoverClientError::GraphQl { .. } => (None, None),
                RoverClientError::IntrospectionError { .. } => (None, Some(Code::E011)),
                RoverClientError::ClientError { .. } => (None, Some(Code::E012)),
                RoverClientError::InvalidKey => (Some(Suggestion::CheckKey), Some(Code::E013)),
                RoverClientError::MalformedKey => (Some(Suggestion::ProperKey), Some(Code::E014)),
                RoverClientError::UnparseableReleaseVersion { source: _ } => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E015))
                }
                RoverClientError::BadReleaseUrl => (Some(Suggestion::SubmitIssue), None),
                RoverClientError::NoCompositionPublishes { .. } => {
                    (Some(Suggestion::RunComposition), Some(Code::E027))
                }
                RoverClientError::AdhocError { .. } => (None, None),
                RoverClientError::CouldNotConnect { .. } => {
                    (Some(Suggestion::CheckServerConnection), Some(Code::E028))
                }
                RoverClientError::InvalidGraphRef { .. } => {
                    unreachable!("Graph ref parse errors should be caught via structopt")
                }
                RoverClientError::InvalidValidationPeriodDuration(_)
                | RoverClientError::ValidationPeriodTooGranular => {
                    unreachable!("Validation period parse errors should be caught via structopt")
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
                HoustonProblem::CouldNotCreateConfigHome(_) => {
                    (Some(Suggestion::SetConfigHome), Some(Code::E016))
                }
                HoustonProblem::DefaultConfigDirNotFound => {
                    (Some(Suggestion::SetConfigHome), Some(Code::E017))
                }
                HoustonProblem::InvalidOverrideConfigDir(_) => {
                    (Some(Suggestion::SetConfigHome), Some(Code::E018))
                }
                HoustonProblem::NoConfigFound(_) => {
                    let code = Some(Code::E019);
                    let suggestion = if env::var_os(RoverEnvKey::ConfigHome.to_string()).is_some() {
                        Some(Suggestion::MigrateConfigHomeOrCreateConfig)
                    } else {
                        Some(Suggestion::CreateConfig)
                    };
                    (suggestion, code)
                }
                HoustonProblem::NoConfigProfiles => {
                    (Some(Suggestion::NewUserNoProfiles), Some(Code::E020))
                }
                HoustonProblem::ProfileNotFound(_) => {
                    (Some(Suggestion::ListProfiles), Some(Code::E021))
                }
                HoustonProblem::NoNonSensitiveConfigFound(_) => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E022))
                }
                HoustonProblem::PathNotUtf8(_) => (Some(Suggestion::SubmitIssue), Some(Code::E023)),
                HoustonProblem::TomlDeserialization(_) => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E024))
                }
                HoustonProblem::TomlSerialization(_) => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E025))
                }
                HoustonProblem::IoError(_) => (Some(Suggestion::SubmitIssue), Some(Code::E026)),
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
