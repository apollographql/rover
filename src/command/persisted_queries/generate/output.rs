use camino::Utf8PathBuf;
use serde::Serialize;

use crate::command::output::CliOutput;

#[derive(Debug, Serialize)]
pub struct GenerateOutput {
    pub path: Utf8PathBuf,
    pub operation_count: usize,
}

impl CliOutput for GenerateOutput {
    fn text(&self) -> String {
        format!(
            "Manifest written to {} with {} operation{}.",
            self.path,
            self.operation_count,
            if self.operation_count == 1 { "" } else { "s" },
        )
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn text_output_is_singular_for_one_operation() {
        let out = GenerateOutput {
            path: "manifest.json".into(),
            operation_count: 1,
        };
        assert_that!(out.text()).is_equal_to(
            "Manifest written to manifest.json with 1 operation.".to_string(),
        );
    }

    #[test]
    fn text_output_is_plural_for_zero_operations() {
        let out = GenerateOutput {
            path: "manifest.json".into(),
            operation_count: 0,
        };
        assert_that!(out.text()).is_equal_to(
            "Manifest written to manifest.json with 0 operations.".to_string(),
        );
    }

    #[test]
    fn text_output_is_plural_for_multiple_operations() {
        let out = GenerateOutput {
            path: "manifest.json".into(),
            operation_count: 42,
        };
        assert_that!(out.text()).is_equal_to(
            "Manifest written to manifest.json with 42 operations.".to_string(),
        );
    }
}
