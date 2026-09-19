//! What `rover dev` reports when a session ends.

use serde_json::Value;

use crate::command::CliOutput;

/// What a finished `rover dev` session reports.
///
/// A session's value is the router it ran, not anything it prints at the end,
/// so there is nothing to say on stdout. It exists as a type rather than
/// `RoverOutput::EmptySuccess` so that the plugins the session resolved have
/// somewhere to go (FR57), and because `EmptySuccess` is shared with a few
/// dozen unrelated commands that must not grow that field.
#[derive(Debug, Default)]
pub struct DevOutput;

impl CliOutput for DevOutput {
    fn text(&self) -> String {
        String::new()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;
    use crate::{RoverOutput, options::JsonOutput};

    /// This command printed nothing and serialized to a bare success envelope
    /// before it had an output type, and moving onto the trait must not change
    /// either.
    #[test]
    fn matches_what_empty_success_produced() {
        let output = RoverOutput::CliOutput(Box::new(DevOutput));

        assert_that!(output.get_stdout().unwrap()).is_equal_to(None);
        assert_that!(JsonOutput::from(&output).to_string())
            .is_equal_to(JsonOutput::from(&RoverOutput::EmptySuccess).to_string());
    }
}
