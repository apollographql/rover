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
