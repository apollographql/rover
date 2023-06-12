use std::{
    io::{self, BufWriter, Write},
    ops::Range,
};

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
    pub fn print_ariadne(&self) -> io::Result<String> {
        if self.diagnostics.is_empty() {
            Ok("No lint errors found in this schema".to_string())
        } else {
            let mut colors = ColorGenerator::new();
            let error_color = colors.next();
            let warning_color = colors.next();
            let ignored_color = colors.next();
            let file_name = self.file_name.as_str();

            let mut output = BufWriter::new(Vec::new())
                .into_inner()
                // this shouldn't happen because `Vec` is not a fixed size and should grow to whatever we write to it
                .expect("could not write lint report to buffer");

            for (i, diagnostic) in self.diagnostics.iter().enumerate() {
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
                .finish();

                report.write(
                    (file_name, Source::from(self.proposed_schema.as_str())),
                    &mut output,
                )?;

                if i == self.diagnostics.len() - 1 {
                    writeln!(output)?;
                }
            }

            Ok(String::from_utf8(output)
                .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source))?)
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
