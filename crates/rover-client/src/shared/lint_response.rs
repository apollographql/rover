use std::{
    io::{self, BufWriter, Write},
    ops::Range,
};

use ariadne::{Color, ColorGenerator, Label, Report, ReportKind, Source};
use serde::Serialize;
use serde_json::{json, Value};

use rover_std::is_no_color_set;

/// Convert UTF-8 byte offsets to unicode scalar value offsets as in the `str::chars()` iterator.
struct OffsetMapper {
    map: Vec<usize>,
}

impl OffsetMapper {
    fn new(input: &str) -> Self {
        let mut map = vec![usize::MAX; input.len()];
        for (char_index, (byte_index, _char)) in input.char_indices().enumerate() {
            map[byte_index] = char_index;
        }
        Self { map }
    }

    fn map_range(&self, start: usize, end: usize) -> Range<usize> {
        let start = self.map[start];
        let end = self.map[end];
        Range { start, end }
    }
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct LintResponse {
    pub diagnostics: Vec<Diagnostic>,
    pub file_name: String,
    pub proposed_schema: String,
}

impl LintResponse {
    pub fn get_ariadne(&self) -> io::Result<String> {
        if self.diagnostics.is_empty() {
            Ok("No lint violations found in this schema".to_string())
        } else {
            let mut colors = ColorGenerator::new();
            let error_color = colors.next();
            let warning_color = colors.next();
            let ignored_color = colors.next();
            let file_name = self.file_name.as_str();
            let mapper = OffsetMapper::new(&self.proposed_schema);

            let mut output = BufWriter::new(Vec::new())
                .into_inner()
                // this shouldn't happen because `Vec` is not a fixed size and should grow to whatever we write to it
                .expect("could not write lint report to buffer");

            for (i, diagnostic) in self.diagnostics.iter().enumerate() {
                let range =
                    mapper.map_range(diagnostic.start_byte_offset, diagnostic.end_byte_offset);
                let color = if is_no_color_set() {
                    Color::Primary
                } else {
                    match diagnostic.level.as_str() {
                        "ERROR" => error_color,
                        "WARNING" => warning_color,
                        "IGNORED" => ignored_color,
                        &_ => Color::Primary,
                    }
                };
                let report_kind = if is_no_color_set() {
                    ReportKind::Custom(diagnostic.level.as_str(), Color::Primary)
                } else {
                    match diagnostic.level.as_str() {
                        "ERROR" => ReportKind::Error,
                        "WARNING" => ReportKind::Warning,
                        "IGNORED" => ReportKind::Advice,
                        &_ => ReportKind::Advice,
                    }
                };
                let report = Report::build(report_kind, (file_name, range.clone()))
                    .with_message(diagnostic.message.clone())
                    .with_label(
                        Label::new((file_name, range))
                            .with_message(diagnostic.message.clone())
                            .with_color(color),
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
    pub start_line: i64,
    pub start_byte_offset: usize,
    pub end_byte_offset: usize,
    pub rule: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_points_to_correct_place() {
        let input = r#"
"a 멀티바이트 type comment"
type Query {
    key: Int!
}
        "#
        .trim();

        // Response got by running the above schema through linter-grpc locally.
        let mock_response = LintResponse {
            diagnostics: vec![Diagnostic {
                level: "WARNING".to_string(),
                coordinate: "Query.key".to_string(),
                message: "Schema element Query.key is missing a description.".to_string(),
                rule: "DESCRIPTION_MISSING".to_string(),
                start_line: 3,
                start_byte_offset: 50,
                end_byte_offset: 53,
            }],
            file_name: "schema.graphql".to_string(),
            proposed_schema: input.to_string(),
        };

        let s = mock_response.get_ariadne().unwrap();
        assert_eq!(
            strip_ansi_escapes::strip_str(&s),
            r#"Warning: Schema element Query.key is missing a description.
   ╭─[schema.graphql:3:5]
   │
 3 │     key: Int!
   │     ─┬─  
   │      ╰─── Schema element Query.key is missing a description.
───╯

"#
        );
    }
}
