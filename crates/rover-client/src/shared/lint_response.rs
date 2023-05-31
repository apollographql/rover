use prettytable::format::consts::FORMAT_BOX_CHARS;
use serde::Serialize;

use prettytable::{row, Table};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct LintResponse {
    pub diagnostics: Vec<Diagnostic>,
}

impl LintResponse {
    pub fn get_table(&self) -> String {
        let mut table = Table::new();

        table.set_format(*FORMAT_BOX_CHARS);

        // bc => sets top row to be bold and center
        table.add_row(row![bc => "Coordinate", "Line", "Level", "Description"]);

        for diagnostic in &self.diagnostics {
            table.add_row(row![
                diagnostic.coordinate,
                diagnostic.start_line,
                diagnostic.level,
                diagnostic.message,
            ]);
        }

        table.to_string()
    }

    pub fn get_json(&self) -> Value {
        json!(self)
    }
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct Diagnostic {
    pub rule: String,
    pub level: String,
    pub message: String,
    pub coordinate: String,
    pub start_line: u64,
}
