mod code;
mod suggestion;

use code::Code;
use suggestion::Suggestion;

use houston::HoustonProblem;
use rover_client::RoverClientError;

use crate::env::RoverEnvKey;

use std::env;

/// Metadata contains extra information about specific errors
/// Currently this includes an optional error `Code`
/// and an optional `Suggestion`
#[derive(Default, Debug)]
pub struct Metadata {
    pub suggestion: Option<Suggestion>,
    pub code: Option<Code>,
}

/// `Metadata` structs can be created from an `anyhow::Error`
/// This works by downcasting the errors to their underlying types
/// and creating `Suggestion`s and `Code`s where applicable
impl From<&mut anyhow::Error> for Metadata {
    fn from(error: &mut anyhow::Error) -> Self {
        if let Some(rover_client_error) = error.downcast_ref::<RoverClientError>() {
            let (suggestion, code) = match rover_client_error {
                RoverClientError::InvalidJSON(_)
                | RoverClientError::InvalidHeaderName(_)
                | RoverClientError::InvalidHeaderValue(_)
                | RoverClientError::SendRequest(_)
                | RoverClientError::NoCheckData
                | RoverClientError::InvalidSeverity => (Some(Suggestion::SubmitIssue), None),
                _ => (None, None),
            };
            return Metadata { suggestion, code };
        }

        if let Some(houston_problem) = error.downcast_ref::<HoustonProblem>() {
            let (suggestion, code) = match houston_problem {
                HoustonProblem::NoNonSensitiveConfigFound(_) => {
                    (Some(Suggestion::RerunWithSensitive), None)
                }
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
                HoustonProblem::ProfileNotFound(_) => (Some(Suggestion::ListProfiles), None),
                HoustonProblem::TomlDeserialization(_)
                | HoustonProblem::TomlSerialization(_)
                | HoustonProblem::IOError(_) => (Some(Suggestion::SubmitIssue), None),
            };
            return Metadata { suggestion, code };
        }

        Metadata::default()
    }
}
