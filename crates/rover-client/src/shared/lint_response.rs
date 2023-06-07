use std::ops::Range;

use ariadne::{ColorGenerator, Label, Report, ReportKind, Source};
use serde::Serialize;

use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct LintResponse {
    pub diagnostics: Vec<Diagnostic>,
    pub file_name: String,
    pub proposed_schema: String,
}

impl LintResponse {
    pub fn get_ariadne(&self) -> String {
        if self.diagnostics.is_empty() {
            "No lint errors found in this schema".to_owned()
        } else {
            let mut colors = ColorGenerator::new();
            let error_color = colors.next();
            let warning_color = colors.next();
            let ignored_color = colors.next();
            let file_name = self.file_name.as_str();

            let mut report_builder =
                Report::build(ReportKind::Error, file_name, 0).with_message("Linter Results");

            for diagnostic in &self.diagnostics {
                let range = Range {
                    start: diagnostic.start_byte_offset,
                    end: diagnostic.end_byte_offset,
                };
                report_builder.add_label(
                    Label::new((file_name, range))
                        .with_message(format!(
                            "{}: {}",
                            diagnostic.level.clone(),
                            diagnostic.message.clone(),
                        ))
                        .with_color(match diagnostic.level.as_str() {
                            "ERROR" => error_color,
                            "WARNING" => warning_color,
                            "IGNORED" => ignored_color,
                            &_ => colors.next(),
                        }),
                );
            }
            let result = report_builder
                .finish()
                .eprint((file_name, Source::from(self.proposed_schema.as_str())));

            if result.is_ok() {
                String::new()
            } else {
                "Display of results failed".to_owned()
            }
        }
    }

    pub fn get_json(&self) -> Value {
        json!(self)
    }
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct Diagnostic {
    pub level: String,
    pub message: String,
    pub coordinate: String,
    pub start_byte_offset: usize,
    pub end_byte_offset: usize,
}
