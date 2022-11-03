pub(crate) mod code;
mod suggestion;

pub(crate) use code::Code;
pub use suggestion::Suggestion;

use houston::HoustonProblem;
use rover_client::RoverClientError;

use crate::{command::output::JsonVersion, utils::env::RoverEnvKey};

use std::env;

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

    // anyhow's debug implementation prints the error cause, most of the time we want this
    // but sometimes the cause is already included in the error's Display impl (like reqwest::Error)
    #[serde(skip_serializing)]
    pub skip_printing_cause: bool,

    #[serde(skip_serializing)]
    pub(crate) json_version: JsonVersion,
}

/// `Metadata` structs can be created from an `saucer::Error`
/// This works by downcasting the errors to their underlying types
/// and creating `Suggestion`s and `Code`s where applicable
impl From<&mut saucer::Error> for Metadata {
    fn from(error: &mut saucer::Error) -> Self {
        let mut skip_printing_cause = false;
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
                RoverClientError::SendRequest { source, is_studio } => {
                    // reqwest::Error's Display impl already includes the cause, so we can skip printing it
                    skip_printing_cause = true;

                    if source.is_connect() {
                        let code = Some(Code::E028);
                        if *is_studio {
                            (Some(Suggestion::SubmitIssue), code)
                        } else {
                            (Some(Suggestion::CheckServerConnection), code)
                        }
                    } else if source.is_timeout() {
                        (Some(Suggestion::IncreaseClientTimeout), Some(Code::E031))
                    } else if source.is_decode() {
                        if *is_studio {
                            (Some(Suggestion::SubmitIssue), Some(Code::E004))
                        } else {
                            (Some(Suggestion::CheckResponseType), Some(Code::E004))
                        }
                    } else if source.is_status() {
                        if *is_studio {
                            (Some(Suggestion::SubmitIssue), Some(Code::E004))
                        } else {
                            (Some(Suggestion::CheckServerConnection), Some(Code::E004))
                        }
                    } else if *is_studio {
                        (Some(Suggestion::SubmitIssue), Some(Code::E004))
                    } else {
                        (None, Some(Code::E004))
                    }
                }
                RoverClientError::MalformedResponse { null_field: _ } => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E005))
                }
                RoverClientError::InvalidSeverity => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E006))
                }
                RoverClientError::SubgraphBuildErrors {
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
                RoverClientError::BuildErrors {
                    source: _,
                    num_subgraphs,
                } => (
                    Some(Suggestion::FixCompositionErrors {
                        num_subgraphs: *num_subgraphs,
                    }),
                    Some(Code::E029),
                ),
                RoverClientError::OperationCheckFailure {
                    graph_ref,
                    check_response: _,
                } => (
                    Some(Suggestion::FixOperationsInSchema {
                        graph_ref: graph_ref.clone(),
                    }),
                    Some(Code::E030),
                ),
                RoverClientError::DownstreamCheckFailure {
                    blocking_downstream_variants: _,
                    target_url,
                } => (
                    Some(Suggestion::FixDownstreamCheckFailure {
                        target_url: target_url.clone(),
                    }),
                    Some(Code::E037),
                ),
                RoverClientError::OtherCheckTaskFailure {
                    has_build_task: _,
                    has_downstream_task: _,
                    target_url,
                } => (
                    Some(Suggestion::FixOtherCheckTaskFailure {
                        target_url: target_url.clone(),
                    }),
                    Some(Code::E036),
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
                RoverClientError::InvalidKey => {
                    let suggestion_key = match std::env::var(RoverEnvKey::Key.to_string()) {
                        Ok(_) => Suggestion::TryUnsetKey,
                        Err(_) => Suggestion::CheckKey,
                    };

                    (Some(suggestion_key), Some(Code::E013))
                }
                RoverClientError::MalformedKey => (Some(Suggestion::ProperKey), Some(Code::E014)),
                RoverClientError::UnparseableReleaseVersion { source: _ } => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E015))
                }
                RoverClientError::BadReleaseUrl => (Some(Suggestion::SubmitIssue), None),
                RoverClientError::NoSupergraphBuilds { .. } => {
                    (Some(Suggestion::RunComposition), Some(Code::E027))
                }
                RoverClientError::AdhocError { .. } => (None, None),
                RoverClientError::InvalidGraphRef { .. } => {
                    unreachable!("Graph ref parse errors should be caught via clap")
                }
                RoverClientError::InvalidValidationPeriodDuration(_)
                | RoverClientError::ValidationPeriodTooGranular => {
                    unreachable!("Validation period parse errors should be caught via clap")
                }
                RoverClientError::InvalidInputError { graph_ref } => (
                    Some(Suggestion::FixChecksInput {
                        graph_ref: graph_ref.clone(),
                    }),
                    Some(Code::E032),
                ),
                RoverClientError::PermissionError { .. } => {
                    (Some(Suggestion::CheckGraphNameAndAuth), Some(Code::E033))
                }
                RoverClientError::PlanError { .. } => {
                    (Some(Suggestion::UpgradePlan), Some(Code::E034))
                }
                RoverClientError::ChecksTimeoutError { url } => (
                    Some(Suggestion::IncreaseChecksTimeout { url: url.clone() }),
                    None,
                ),
            };
            return Metadata {
                json_version: JsonVersion::default(),
                suggestion,
                code,
                skip_printing_cause,
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
                HoustonProblem::CorruptedProfile(profile_name) => (
                    Some(Suggestion::RecreateConfig(profile_name.to_string())),
                    Some(Code::E035),
                ),
                HoustonProblem::PathNotUtf8(_) => (Some(Suggestion::SubmitIssue), Some(Code::E023)),
                HoustonProblem::TomlDeserialization(_) => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E024))
                }
                HoustonProblem::TomlSerialization(_) => {
                    (Some(Suggestion::SubmitIssue), Some(Code::E025))
                }
                HoustonProblem::IoError(_) => (Some(Suggestion::SubmitIssue), Some(Code::E026)),
                HoustonProblem::SaucerError(_) => (None, None),
            };
            return Metadata {
                json_version: JsonVersion::default(),
                suggestion,
                code,
                skip_printing_cause,
            };
        }

        Metadata::default()
    }
}
