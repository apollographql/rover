use std::collections::HashMap;

use apollo_compiler::{Node, ast, parser::Parser as ApolloParser};
use camino::{Utf8Path, Utf8PathBuf};
use thiserror::Error;

use crate::command::client::extensions::ExtensionSnippet;

#[derive(Debug, Error)]
pub(super) enum ParsedFileError {
    #[error("{0}")]
    Syntax(String),
    #[error("anonymous operations are not supported; all operations must have a name")]
    AnonymousOperation,
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
        let doc = ApolloParser::new()
            .parse_ast(contents, file.as_std_path())
            .map_err(|e| ParsedFileError::Syntax(e.to_string()))?;

        let mut extensions = Vec::new();
        let mut fragments = HashMap::new();
        let mut operations = Vec::new();

        for definition in &doc.definitions {
            match definition {
                ast::Definition::FragmentDefinition(fragment) => {
                    let name = fragment.name.to_string();
                    if let Some(span) = fragment.location()
                        && let Some(text) = contents.get(span.offset()..span.end_offset())
                    {
                        fragments.insert(name, text.to_string());
                    }
                }
                ast::Definition::OperationDefinition(op) => {
                    if op.name.is_none() {
                        return Err(ParsedFileError::AnonymousOperation);
                    }
                    if let Some(op_input) = OperationInput::new(file, contents, op, &doc.sources) {
                        operations.push(op_input);
                    }
                }
                other => {
                    if let Some(span) = type_system_definition_span(other)
                        && let Some(text) = contents.get(span.offset()..span.end_offset())
                    {
                        extensions.push(ExtensionSnippet {
                            text: text.to_string(),
                            file: file.to_path_buf(),
                        });
                    }
                }
            }
        }

        Ok(Self {
            operations,
            extensions,
            fragments,
        })
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
    fn new(
        file: &Utf8Path,
        contents: &str,
        op: &Node<ast::OperationDefinition>,
        sources: &apollo_compiler::parser::SourceMap,
    ) -> Option<Self> {
        let name = op.name.as_ref()?.to_string();
        let span = op.location()?;
        let body = contents.get(span.offset()..span.end_offset())?.to_string();
        let loc = op.line_column_range(sources)?;
        Some(Self {
            name,
            body,
            file: file.to_path_buf(),
            line: loc.start.line,
            column: loc.start.column,
        })
    }
}

fn type_system_definition_span(
    def: &ast::Definition,
) -> Option<apollo_compiler::parser::SourceSpan> {
    use ast::Definition::*;
    match def {
        DirectiveDefinition(n) => n.location(),
        SchemaDefinition(n) => n.location(),
        ScalarTypeDefinition(n) => n.location(),
        ObjectTypeDefinition(n) => n.location(),
        InterfaceTypeDefinition(n) => n.location(),
        UnionTypeDefinition(n) => n.location(),
        EnumTypeDefinition(n) => n.location(),
        InputObjectTypeDefinition(n) => n.location(),
        SchemaExtension(n) => n.location(),
        ScalarTypeExtension(n) => n.location(),
        ObjectTypeExtension(n) => n.location(),
        InterfaceTypeExtension(n) => n.location(),
        UnionTypeExtension(n) => n.location(),
        EnumTypeExtension(n) => n.location(),
        InputObjectTypeExtension(n) => n.location(),
        OperationDefinition(_) | FragmentDefinition(_) => unreachable!(),
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

    /// Verifies that a file with GraphQL syntax errors returns a Syntax parse error.
    #[rstest]
    fn syntax_error_returns_err(file: Utf8PathBuf) {
        let result = ParsedFile::new(&file, "not { valid !!!");
        assert!(matches!(result, Err(ParsedFileError::Syntax(_))));
    }

    /// Verifies that a named operation is collected with its name, body, file path, and source
    /// location.
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

    /// Verifies that an anonymous (unnamed) operation returns an error, matching the behavior of
    /// the original Apollo CLI.
    #[rstest]
    fn anonymous_operation_returns_error(file: Utf8PathBuf) {
        let result = ParsedFile::new(&file, "{ __typename }");
        assert!(matches!(result, Err(ParsedFileError::AnonymousOperation)));
    }

    /// Verifies that all named operations in a multi-operation file are individually collected.
    #[rstest]
    fn multiple_named_operations_all_collected(file: Utf8PathBuf) {
        let pf = ParsedFile::new(&file, "query A { __typename }\nquery B { __typename }").unwrap();
        assert_eq!(pf.operations.len(), 2);
        let names: Vec<&str> = pf.operations.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"A"));
        assert!(names.contains(&"B"));
    }

    /// Verifies that a fragment definition is stored in the fragments map keyed by its name,
    /// rather than the operations list.
    #[rstest]
    fn fragment_goes_into_fragments_map(file: Utf8PathBuf) {
        let pf = ParsedFile::new(&file, "fragment MyFragment on Query { __typename }").unwrap();
        assert!(pf.operations.is_empty());
        assert!(pf.extensions.is_empty());
        let fragment_body = pf
            .fragments
            .get("MyFragment")
            .expect("fragment should be keyed by its name");
        assert!(fragment_body.contains("fragment MyFragment"));
    }

    /// Verifies that a schema extension definition is stored in the extensions list rather than
    /// operations.
    #[rstest]
    fn schema_extension_goes_into_extensions(file: Utf8PathBuf) {
        let pf = ParsedFile::new(&file, "extend type Query { world: String }").unwrap();
        assert!(pf.operations.is_empty());
        assert!(pf.fragments.is_empty());
        assert_eq!(pf.extensions.len(), 1);
        assert!(pf.extensions[0].text.contains("extend type Query"));
        assert_eq!(pf.extensions[0].file, file);
    }

    /// Verifies that a file with operations, fragments, and extensions is correctly split into the
    /// three respective buckets.
    #[rstest]
    fn mixed_content_split_into_correct_buckets(file: Utf8PathBuf) {
        let content = indoc::indoc! {"
            query Hello { __typename }
            fragment MyFragment on Query { __typename }
            extend type Query { world: String }
        "};
        let pf = ParsedFile::new(&file, content).unwrap();
        assert_eq!(pf.operations.len(), 1);
        assert_eq!(pf.operations[0].name, "Hello");
        assert!(pf.fragments.contains_key("MyFragment"));
        assert_eq!(pf.extensions.len(), 1);
    }

    /// Verifies that an operation starting on the second line of a file reports line=2.
    #[rstest]
    fn operation_on_second_line_has_correct_location(file: Utf8PathBuf) {
        let content = "# comment\nquery Hello { __typename }";
        let pf = ParsedFile::new(&file, content).unwrap();
        assert_eq!(pf.operations[0].line, 2);
        assert_eq!(pf.operations[0].column, 1);
    }

    /// Verifies that an indented operation's column reflects its distance from the start of the
    /// line.
    #[rstest]
    fn indented_operation_has_correct_column(file: Utf8PathBuf) {
        let content = "# comment\n  query Hello { __typename }";
        let pf = ParsedFile::new(&file, content).unwrap();
        assert_eq!(pf.operations[0].line, 2);
        assert_eq!(pf.operations[0].column, 3);
    }
}
