use apollo_compiler::Schema;
use camino::Utf8PathBuf;

#[derive(Debug, Clone)]
pub struct ExtensionSnippet {
    pub text: String,
    pub file: Utf8PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionFailure {
    pub file: Utf8PathBuf,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

pub fn validate_extensions(
    base_sdl: &str,
    extensions: &[ExtensionSnippet],
) -> Vec<ExtensionFailure> {
    if extensions.is_empty() {
        return Vec::new();
    }

    let mut combined = base_sdl.to_string();
    let mut newline_count = count_newlines(base_sdl);
    let mut extension_ranges = Vec::with_capacity(extensions.len());
    for ext in extensions {
        combined.push_str("\n\n");
        newline_count += 2;

        let start_line = newline_count + 1;
        let ext_newlines = count_newlines(&ext.text);
        let end_line = start_line + ext_newlines;

        extension_ranges.push((start_line, end_line, ext.file.clone()));

        combined.push_str(&ext.text);
        newline_count += ext_newlines;
    }

    let base_file = Utf8PathBuf::from("schema.graphql");
    let default_extension_file = extensions
        .first()
        .map(|e| e.file.clone())
        .unwrap_or_else(|| base_file.clone());

    match Schema::parse_and_validate(combined, "schema.graphql") {
        Ok(schema) => schema
            .errors
            .iter()
            .map(|diag| {
                let (line, column) = diag
                    .location()
                    .and_then(|loc| loc.line_column_range(&schema.sources))
                    .map(|range| (Some(range.start.line), Some(range.start.column)))
                    .unwrap_or((None, None));

                let (file, mapped_line) = match line {
                    Some(line) => extension_ranges
                        .iter()
                        .find(|(start, end, _)| line >= *start && line <= *end)
                        .map(|(start, _, file)| (file.clone(), Some(line - start + 1)))
                        .unwrap_or_else(|| (base_file.clone(), Some(line))),
                    None => (default_extension_file.clone(), None),
                };

                ExtensionFailure {
                    file,
                    message: diag.to_string(),
                    line: mapped_line,
                    column,
                }
            })
            .collect(),
        Err(errs) => vec![ExtensionFailure {
            file: default_extension_file,
            message: errs.to_string(),
            line: None,
            column: None,
        }],
    }
}

fn count_newlines(text: &str) -> usize {
    text.bytes().filter(|b| *b == b'\n').count()
}
