mod code;
mod suggestion;

pub use code::RoverErrorCode;
pub use suggestion::RoverErrorSuggestion;

use houston::HoustonProblem;
use rover_client::RoverClientError;

use crate::{command::output::JsonVersion, utils::env::RoverEnvKey};

use std::env;

use serde::Serialize;

/// Metadata contains extra information about specific errors
/// Currently this includes an optional error `Code`
/// and an optional `Suggestion`
#[derive(Default, Serialize, Debug)]
pub struct RoverErrorMetadata {
    // skip serializing for now until we can appropriately strip color codes
    #[serde(skip_serializing)]
    pub suggestion: Option<RoverErrorSuggestion>,
    pub code: Option<RoverErrorCode>,

    // anyhow's debug implementation prints the error cause, most of the time we want this
    // but sometimes the cause is already included in the error's Display impl (like reqwest::Error)
    #[serde(skip_serializing)]
    pub skip_printing_cause: bool,

    #[serde(skip_serializing)]
    pub(crate) json_version: JsonVersion,
}

/// `Metadata` structs can be created from an `anyhow::Error`
/// This works by downcasting the errors to their underlying types
/// and creating `Suggestion`s and `Code`s where applicable
impl From<&mut anyhow::Error> for RoverErrorMetadata {
    fn from(error: &mut anyhow::Error) -> Self {
        let mut skip_printing_cause = false;
        if let Some(rover_client_error) = error.downcast_ref::<RoverClientError>() {
            let (suggestion, code) = match rover_client_error {
                RoverClientError::InvalidJson(_) => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E001),
                ),
                RoverClientError::InvalidHeaderName(_) => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E002),
                ),
                RoverClientError::InvalidHeaderValue(_) => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E003),
                ),
                RoverClientError::SendRequest { source, is_studio } => {
                    // reqwest::Error's Display impl already includes the cause, so we can skip printing it
                    skip_printing_cause = true;

                    if source.is_connect() {
                        let code = Some(RoverErrorCode::E028);
                        if *is_studio {
                            (Some(RoverErrorSuggestion::SubmitIssue), code)
                        } else {
                            (Some(RoverErrorSuggestion::CheckServerConnection), code)
                        }
                    } else if source.is_timeout() {
                        (
                            Some(RoverErrorSuggestion::IncreaseClientTimeout),
                            Some(RoverErrorCode::E031),
                        )
                    } else if source.is_decode() {
                        if *is_studio {
                            (
                                Some(RoverErrorSuggestion::SubmitIssue),
                                Some(RoverErrorCode::E004),
                            )
                        } else {
                            (
                                Some(RoverErrorSuggestion::CheckResponseType),
                                Some(RoverErrorCode::E004),
                            )
                        }
                    } else if source.is_status() {
                        if *is_studio {
                            (
                                Some(RoverErrorSuggestion::SubmitIssue),
                                Some(RoverErrorCode::E004),
                            )
                        } else {
                            (
                                Some(RoverErrorSuggestion::CheckServerConnection),
                                Some(RoverErrorCode::E004),
                            )
                        }
                    } else if *is_studio {
                        (
                            Some(RoverErrorSuggestion::SubmitIssue),
                            Some(RoverErrorCode::E004),
                        )
                    } else {
                        (None, Some(RoverErrorCode::E004))
                    }
                }
                RoverClientError::MalformedResponse { null_field: _ } => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E005),
                ),
                RoverClientError::InvalidSeverity => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E006),
                ),
                RoverClientError::SubgraphBuildErrors {
                    graph_ref,
                    subgraph,
                    source: _,
                } => (
                    Some(RoverErrorSuggestion::FixSubgraphSchema {
                        graph_ref: graph_ref.clone(),
                        subgraph: subgraph.clone(),
                    }),
                    Some(RoverErrorCode::E029),
                ),
                RoverClientError::BuildErrors {
                    source,
                    num_subgraphs,
                } => {
                    if source.is_config {
                        (
                            Some(RoverErrorSuggestion::FixSupergraphConfigErrors),
                            Some(RoverErrorCode::E038),
                        )
                    } else {
                        (
                            Some(RoverErrorSuggestion::FixCompositionErrors {
                                num_subgraphs: *num_subgraphs,
                            }),
                            Some(RoverErrorCode::E029),
                        )
                    }
                }
                RoverClientError::OperationCheckFailure {
                    graph_ref,
                    check_response: _,
                } => (
                    Some(RoverErrorSuggestion::FixOperationsInSchema {
                        graph_ref: graph_ref.clone(),
                    }),
                    Some(RoverErrorCode::E030),
                ),
                RoverClientError::DownstreamCheckFailure {
                    blocking_downstream_variants: _,
                    target_url,
                } => (
                    Some(RoverErrorSuggestion::FixDownstreamCheckFailure {
                        target_url: target_url.clone(),
                    }),
                    Some(RoverErrorCode::E037),
                ),
                RoverClientError::OtherCheckTaskFailure {
                    has_build_task: _,
                    has_downstream_task: _,
                    target_url,
                } => (
                    Some(RoverErrorSuggestion::FixOtherCheckTaskFailure {
                        target_url: target_url.clone(),
                    }),
                    Some(RoverErrorCode::E036),
                ),
                RoverClientError::SubgraphIntrospectionNotAvailable => (
                    Some(RoverErrorSuggestion::UseFederatedGraph),
                    Some(RoverErrorCode::E007),
                ),
                RoverClientError::ExpectedFederatedGraph {
                    graph_ref: _,
                    can_operation_convert,
                } => {
                    if *can_operation_convert {
                        (
                            Some(RoverErrorSuggestion::ConvertGraphToSubgraph),
                            Some(RoverErrorCode::E007),
                        )
                    } else {
                        (
                            Some(RoverErrorSuggestion::UseFederatedGraph),
                            Some(RoverErrorCode::E007),
                        )
                    }
                }
                RoverClientError::NoSchemaForVariant {
                    graph_ref,
                    valid_variants,
                    frontend_url_root,
                } => (
                    Some(RoverErrorSuggestion::ProvideValidVariant {
                        graph_ref: graph_ref.clone(),
                        valid_variants: valid_variants.clone(),
                        frontend_url_root: frontend_url_root.clone(),
                    }),
                    Some(RoverErrorCode::E008),
                ),
                RoverClientError::NoSubgraphInGraph {
                    invalid_subgraph: _,
                    valid_subgraphs,
                } => (
                    Some(RoverErrorSuggestion::ProvideValidSubgraph(
                        valid_subgraphs.clone(),
                    )),
                    Some(RoverErrorCode::E009),
                ),
                RoverClientError::GraphNotFound { .. } => (
                    Some(RoverErrorSuggestion::CheckGraphNameAndAuth),
                    Some(RoverErrorCode::E010),
                ),
                RoverClientError::GraphQl { .. } => (None, None),
                RoverClientError::IntrospectionError { .. } => (None, Some(RoverErrorCode::E011)),
                RoverClientError::ClientError { .. } => (None, Some(RoverErrorCode::E012)),
                RoverClientError::InvalidKey => {
                    let suggestion_key = match std::env::var(RoverEnvKey::Key.to_string()) {
                        Ok(_) => RoverErrorSuggestion::TryUnsetKey,
                        Err(_) => RoverErrorSuggestion::CheckKey,
                    };

                    (Some(suggestion_key), Some(RoverErrorCode::E013))
                }
                RoverClientError::MalformedKey => (
                    Some(RoverErrorSuggestion::ProperKey),
                    Some(RoverErrorCode::E014),
                ),
                RoverClientError::UnparseableReleaseVersion { source: _ } => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E015),
                ),
                RoverClientError::BadReleaseUrl => (Some(RoverErrorSuggestion::SubmitIssue), None),
                RoverClientError::NoSupergraphBuilds { .. } => (
                    Some(RoverErrorSuggestion::RunComposition),
                    Some(RoverErrorCode::E027),
                ),
                RoverClientError::AdhocError { .. } => (None, None),
                RoverClientError::InvalidGraphRef { .. } => {
                    unreachable!("Graph ref parse errors should be caught via clap")
                }
                RoverClientError::InvalidValidationPeriodDuration(_)
                | RoverClientError::ValidationPeriodTooGranular => {
                    unreachable!("Validation period parse errors should be caught via clap")
                }
                RoverClientError::InvalidInputError { graph_ref } => (
                    Some(RoverErrorSuggestion::FixChecksInput {
                        graph_ref: graph_ref.clone(),
                    }),
                    Some(RoverErrorCode::E032),
                ),
                RoverClientError::PermissionError { .. } => (
                    Some(RoverErrorSuggestion::CheckGraphNameAndAuth),
                    Some(RoverErrorCode::E033),
                ),
                RoverClientError::PlanError { .. } => (
                    Some(RoverErrorSuggestion::UpgradePlan),
                    Some(RoverErrorCode::E034),
                ),
                RoverClientError::ChecksTimeoutError { url } => (
                    Some(RoverErrorSuggestion::IncreaseChecksTimeout { url: url.clone() }),
                    None,
                ),
            };
            return RoverErrorMetadata {
                json_version: JsonVersion::default(),
                suggestion,
                code,
                skip_printing_cause,
            };
        }

        if let Some(houston_problem) = error.downcast_ref::<HoustonProblem>() {
            let (suggestion, code) = match houston_problem {
                HoustonProblem::CouldNotCreateConfigHome(_) => (
                    Some(RoverErrorSuggestion::SetConfigHome),
                    Some(RoverErrorCode::E016),
                ),
                HoustonProblem::DefaultConfigDirNotFound => (
                    Some(RoverErrorSuggestion::SetConfigHome),
                    Some(RoverErrorCode::E017),
                ),
                HoustonProblem::InvalidOverrideConfigDir(_) => (
                    Some(RoverErrorSuggestion::SetConfigHome),
                    Some(RoverErrorCode::E018),
                ),
                HoustonProblem::NoConfigFound(_) => {
                    let code = Some(RoverErrorCode::E019);
                    let suggestion = if env::var_os(RoverEnvKey::ConfigHome.to_string()).is_some() {
                        Some(RoverErrorSuggestion::MigrateConfigHomeOrCreateConfig)
                    } else {
                        Some(RoverErrorSuggestion::CreateConfig)
                    };
                    (suggestion, code)
                }
                HoustonProblem::NoConfigProfiles => (
                    Some(RoverErrorSuggestion::NewUserNoProfiles),
                    Some(RoverErrorCode::E020),
                ),
                HoustonProblem::ProfileNotFound(_) => (
                    Some(RoverErrorSuggestion::ListProfiles),
                    Some(RoverErrorCode::E021),
                ),
                HoustonProblem::NoNonSensitiveConfigFound(_) => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E022),
                ),
                HoustonProblem::CorruptedProfile(profile_name) => (
                    Some(RoverErrorSuggestion::RecreateConfig(
                        profile_name.to_string(),
                    )),
                    Some(RoverErrorCode::E035),
                ),
                HoustonProblem::PathNotUtf8(_) => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E023),
                ),
                HoustonProblem::TomlDeserialization(_) => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E024),
                ),
                HoustonProblem::TomlSerialization(_) => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E025),
                ),
                HoustonProblem::IoError(_) => (
                    Some(RoverErrorSuggestion::SubmitIssue),
                    Some(RoverErrorCode::E026),
                ),
                HoustonProblem::AdhocError(_) => (None, None),
            };
            return RoverErrorMetadata {
                json_version: JsonVersion::default(),
                suggestion,
                code,
                skip_printing_cause,
            };
        }

        RoverErrorMetadata::default()
    }
}
