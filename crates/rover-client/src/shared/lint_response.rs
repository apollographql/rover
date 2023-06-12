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
    pub fn print_ariadne(&self) -> String {
        if self.diagnostics.is_empty() {
            "No lint errors found in this schema".to_owned()
        } else {
            let mut colors = ColorGenerator::new();
            let error_color = colors.next();
            let warning_color = colors.next();
            let ignored_color = colors.next();
            let file_name = self.file_name.as_str();

            let mut result = true;

            for diagnostic in &self.diagnostics {
                let range = Range {
                    start: diagnostic.start_byte_offset,
                    end: diagnostic.end_byte_offset,
                };
                let report = Report::build(
                    match diagnostic.level.as_str() {
                        "ERROR" => ReportKind::Error,
                        "WARNING" => ReportKind::Warning,
                        "IGNORED" => ReportKind::Advice,
                        &_ => ReportKind::Advice,
                    },
                    file_name,
                    0,
                )
                .with_label(
                    Label::new((file_name, range))
                        .with_message(diagnostic.message.clone())
                        .with_color(match diagnostic.level.as_str() {
                            "ERROR" => error_color,
                            "WARNING" => warning_color,
                            "IGNORED" => ignored_color,
                            &_ => colors.next(),
                        }),
                )
                .finish()
                .eprint((file_name, Source::from(self.proposed_schema.as_str())));
                if report.is_err() {
                    result = false;
                }
            }

            if result {
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
