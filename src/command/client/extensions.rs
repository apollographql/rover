use std::sync::Arc;

use apollo_compiler::Schema;
use apollo_compiler::diagnostic::ToCliReport;
use apollo_compiler::parser::{SourceFile, SourceSpan};
use camino::Utf8PathBuf;

#[derive(Debug, Clone)]
/// A schema-extension snippet extracted from a client `.graphql` file.
pub struct ExtensionSnippet {
    pub text: String,
    pub file: Utf8PathBuf,
}

/// A validation error produced while merging extension snippets into the base schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionFailure {
    pub file: Utf8PathBuf,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// Validate `extensions` against `base_sdl` using `apollo_compiler`.
///
/// Returns one [`ExtensionFailure`] per diagnostic error. Each failure is attributed to
/// the source file that introduced the problem (falling back to `base_source` when the
/// error location is in the base schema).
///
/// Returns an empty `Vec` when there are no extensions or all extensions are valid.
pub fn validate_extensions(
    base_sdl: &str,
    base_source: &str,
    extensions: &[ExtensionSnippet],
) -> Vec<ExtensionFailure> {
    if extensions.is_empty() {
        return Vec::new();
    }

    let builder = extensions.iter().fold(
        Schema::builder().parse(base_sdl, base_source),
        |b, ext| b.parse(&ext.text, ext.file.as_std_path()),
    );

    let errors = match builder.build() {
        Ok(schema) => match schema.validate() {
            Ok(_) => return Vec::new(),
            Err(e) => e.errors,
        },
        Err(e) => e.errors,
    };

    errors
        .iter()
        .map(|diag| {
            let location = diag.error.location();

            let file = location
                .and_then(|span: SourceSpan| diag.sources.get(&span.file_id()))
                .map(|sf: &Arc<SourceFile>| Utf8PathBuf::from(sf.path().to_string_lossy().as_ref()))
                .unwrap_or_else(|| Utf8PathBuf::from(base_source));

            let (line, column) = diag
                .line_column_range()
                .map(|r| (Some(r.start.line), Some(r.start.column)))
                .unwrap_or((None, None));

            ExtensionFailure {
                file,
                message: diag.to_string(),
                line,
                column,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ext(text: &str, file: &str) -> ExtensionSnippet {
        ExtensionSnippet {
            text: text.to_string(),
            file: Utf8PathBuf::from(file),
        }
    }

    #[test]
    fn valid_extension_returns_no_failures() {
        let failures = validate_extensions(
            "type Query { hello: String }",
            "graph@current",
            &[ext("extend type Query { world: String }", "extensions.graphql")],
        );
        assert!(failures.is_empty());
    }

    #[test]
    fn invalid_extension_attributed_to_extension_file() {
        let failures = validate_extensions(
            "type Query { hello: String }",
            "graph@current",
            &[ext(
                "extend type Query { world: FakeType! }",
                "extensions.graphql",
            )],
        );
        assert!(!failures.is_empty());
        assert_eq!(failures[0].file, Utf8PathBuf::from("extensions.graphql"));
    }

    #[test]
    fn error_in_base_schema_attributed_to_base_source() {
        let failures = validate_extensions(
            "type Query { hello: NonExistentType }",
            "graph@current",
            &[ext("extend type Query { world: String }", "extensions.graphql")],
        );
        assert!(!failures.is_empty());
        assert_eq!(failures[0].file, Utf8PathBuf::from("graph@current"));
    }

    #[test]
    fn empty_extensions_returns_no_failures() {
        let failures = validate_extensions("type Query { hello: String }", "graph@current", &[]);
        assert!(failures.is_empty());
    }
}
