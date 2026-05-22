use itertools::Itertools;
use rover_schema::{SearchResult, root_paths::RootPath};
use serde::Serialize;

use crate::command::CliOutput;

#[derive(Debug, Serialize)]
pub struct SearchOutput {
    pub query: String,
    pub results: Vec<SearchResult>,
}

impl CliOutput for SearchOutput {
    fn text(&self) -> String {
        if self.results.is_empty() {
            return format!("No results for \"{}\"", self.query);
        }

        let header = format!(
            "{} result{} for \"{}\"",
            self.results.len(),
            if self.results.len() == 1 { "" } else { "s" },
            self.query
        );

        let items = self.results.iter().map(format_result).join("\n\n");

        format!("{header}\n\n{items}")
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

fn format_result(r: &SearchResult) -> String {
    let first_line = match &r.description {
        Some(desc) => format!("{} — {}", r.coordinate, desc),
        None => r.coordinate.to_string(),
    };

    let kind_line = if r.via.is_empty() {
        format!("  {}", r.kind)
    } else {
        let paths = r.via.iter().map(format_root_path).join(", ");
        format!("  {}  ·  via {}", r.kind, paths)
    };

    format!("{first_line}\n{kind_line}")
}

fn format_root_path(p: &RootPath) -> String {
    p.segments
        .iter()
        .map(|s| format!("{}.{}", s.type_name, s.field_name))
        .join(" -> ")
}
