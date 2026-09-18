use serde_json::Value;

use crate::command::CliOutput;

/// What `rover lsp` reports when the language server stops.
///
/// The server talks to its editor over the LSP protocol on stdin and stdout
/// for the whole of its life, so there is nothing for it to print as a final
/// payload — and stdout in particular is the protocol's, not ours. This type
/// exists so the session has an output of its own rather than borrowing
/// `RoverOutput::EmptySuccess` from several dozen unrelated commands.
#[derive(Debug, Default)]
pub struct LspOutput;

impl CliOutput for LspOutput {
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

    /// Nothing on stdout is not a detail here: the language server has been
    /// using stdout to speak LSP, so a stray line would be a protocol error.
    #[test]
    fn matches_what_empty_success_produced() {
        let output = RoverOutput::CliOutput(Box::new(LspOutput));

        assert_that!(output.get_stdout().unwrap()).is_equal_to(None);
        assert_that!(JsonOutput::from(&output).to_string())
            .is_equal_to(JsonOutput::from(&RoverOutput::EmptySuccess).to_string());
    }
}
