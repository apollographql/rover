use std::collections::HashMap;

use apollo_parser::{
    Parser,
    cst::{self, CstNode},
};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use thiserror::Error;

use crate::command::client::extensions::ExtensionSnippet;

#[derive(Debug, Error)]
pub(super) enum ParsedFileError {
    #[error("GraphQL syntax errors:\n  {}", .0.iter().join("\n  "))]
    Syntax(Vec<apollo_parser::Error>),
}

#[derive(Debug, Clone)]
pub(super) struct ParsedFile {
    pub(super) operations: Vec<OperationInput>,
    pub(super) extensions: Vec<ExtensionSnippet>,
    /// Fragment definitions keyed by name, for global deduplication across files.
    pub(super) fragments: HashMap<String, String>,
}

impl ParsedFile {
    pub(super) fn new(file: &Utf8Path, contents: &str) -> Result<Self, ParsedFileError> {
        let parser = Parser::new(contents);
        let tree = parser.parse();
        let errors: Vec<_> = tree.errors().collect();
        if !errors.is_empty() {
            let errors = errors.into_iter().cloned().collect();
            return Err(ParsedFileError::Syntax(errors));
        }

        let doc = tree.document();
        let mut extensions = Vec::new();
        let mut fragments = HashMap::new();
        for definition in doc.definitions() {
            if let cst::Definition::FragmentDefinition(ref fragment) = definition {
                let name = fragment
                    .fragment_name()
                    .and_then(|n| n.name())
                    .map(|n| n.syntax().text().to_string());
                if let Some(name) = name {
                    let range = definition.syntax().text_range();
                    let start: usize = range.start().into();
                    let end: usize = range.end().into();
                    if let Some(text) = contents.get(start..end) {
                        fragments.insert(name, text.to_string());
                    }
                }
            } else if !matches!(definition, cst::Definition::OperationDefinition(_)) {
                let range = definition.syntax().text_range();
                let start: usize = range.start().into();
                let end: usize = range.end().into();
                if let Some(text) = contents.get(start..end) {
                    extensions.push(ExtensionSnippet {
                        text: text.to_string(),
                        file: file.to_path_buf(),
                    });
                }
            }
        }

        let mut operations = Vec::new();
        for definition in doc.definitions() {
            if let cst::Definition::OperationDefinition(def) = definition
                && let Some(op) = OperationInput::new(file, contents, def)
            {
                operations.push(op);
            }
        }

        Ok(Self {
            operations,
            extensions,
            fragments,
        })
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use rstest::{fixture, rstest};

    use super::*;

    #[fixture]
    fn file() -> Utf8PathBuf {
        Utf8PathBuf::from("test.graphql")
    }

    #[rstest]
    fn syntax_error_returns_err(file: Utf8PathBuf) {
        let result = ParsedFile::new(&file, "not { valid !!!");
        assert!(matches!(result, Err(ParsedFileError::Syntax(_))));
    }

    #[rstest]
    fn named_operation_collected_with_metadata(file: Utf8PathBuf) {
        let pf = ParsedFile::new(&file, "query Hello { __typename }").unwrap();
        assert_eq!(pf.operations.len(), 1);
        assert_eq!(pf.operations[0].name, "Hello");
        assert_eq!(pf.operations[0].body, "query Hello { __typename }");
        assert_eq!(pf.operations[0].file, file);
        assert_eq!(pf.operations[0].line, 1);
        assert_eq!(pf.operations[0].column, 1);
    }

    #[rstest]
    fn anonymous_operation_is_ignored(file: Utf8PathBuf) {
        let pf = ParsedFile::new(&file, "{ __typename }").unwrap();
        assert!(pf.operations.is_empty());
        assert!(pf.fragments.is_empty());
        assert!(pf.extensions.is_empty());
    }

    #[rstest]
    fn multiple_named_operations_all_collected(file: Utf8PathBuf) {
        let pf = ParsedFile::new(&file, "query A { __typename }\nquery B { __typename }").unwrap();
        assert_eq!(pf.operations.len(), 2);
        let names: Vec<&str> = pf.operations.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"A"));
        assert!(names.contains(&"B"));
    }

    #[rstest]
    fn fragment_goes_into_fragments_map(file: Utf8PathBuf) {
        let pf = ParsedFile::new(&file, "fragment F on Query { __typename }").unwrap();
        assert!(pf.operations.is_empty());
        assert!(pf.extensions.is_empty());
        assert!(pf.fragments.contains_key("F"));
        assert!(pf.fragments["F"].contains("fragment F"));
    }

    #[rstest]
    fn schema_extension_goes_into_extensions(file: Utf8PathBuf) {
        let pf = ParsedFile::new(&file, "extend type Query { world: String }").unwrap();
        assert!(pf.operations.is_empty());
        assert!(pf.fragments.is_empty());
        assert_eq!(pf.extensions.len(), 1);
        assert!(pf.extensions[0].text.contains("extend type Query"));
        assert_eq!(pf.extensions[0].file, file);
    }

    #[rstest]
    fn mixed_content_split_into_correct_buckets(file: Utf8PathBuf) {
        let content = indoc::indoc! {"
            query Hello { __typename }
            fragment F on Query { __typename }
            extend type Query { world: String }
        "};
        let pf = ParsedFile::new(&file, content).unwrap();
        assert_eq!(pf.operations.len(), 1);
        assert_eq!(pf.operations[0].name, "Hello");
        assert!(pf.fragments.contains_key("F"));
        assert_eq!(pf.extensions.len(), 1);
    }

    #[rstest]
    fn operation_on_second_line_has_correct_location(file: Utf8PathBuf) {
        let content = "# comment\nquery Hello { __typename }";
        let pf = ParsedFile::new(&file, content).unwrap();
        assert_eq!(pf.operations[0].line, 2);
        assert_eq!(pf.operations[0].column, 1);
    }

    #[rstest]
    fn indented_operation_has_correct_column(file: Utf8PathBuf) {
        // lines().count() counts the indent spaces as a segment, so line is 3 not 2.
        let content = "# comment\n  query Hello { __typename }";
        let pf = ParsedFile::new(&file, content).unwrap();
        assert_eq!(pf.operations[0].line, 3);
        assert_eq!(pf.operations[0].column, 3);
    }
}

#[derive(Debug, Clone)]
pub(super) struct OperationInput {
    pub(super) name: String,
    pub(super) body: String,
    pub(super) file: Utf8PathBuf,
    pub(super) line: usize,
    pub(super) column: usize,
}

impl OperationInput {
    fn new(file: &Utf8Path, contents: &str, def: cst::OperationDefinition) -> Option<Self> {
        def.name().and_then(|name| {
            let range = def.syntax().text_range();
            let start: usize = range.start().into();
            let end: usize = range.end().into();
            let body = contents.get(start..end)?.to_string();

            let line = contents[..start].lines().count() + 1;
            let column = contents[..start]
                .rsplit_once('\n')
                .map(|(_, rest)| rest.len() + 1)
                .unwrap_or(1);

            Some(Self {
                name: name.syntax().text().to_string(),
                body,
                file: file.to_path_buf(),
                line,
                column,
            })
        })
    }
}
