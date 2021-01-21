mod code;
mod suggestion;

use code::Code;
use suggestion::Suggestion;

use houston::HoustonProblem;
use rover_client::RoverClientError;

#[derive(Default, Debug)]
pub struct Metadata {
    pub suggestion: Option<Suggestion>,
    pub code: Option<Code>,
}

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
                _ => (None, None),
            };
            return Metadata { suggestion, code };
        }

        Metadata::default()
    }
}
