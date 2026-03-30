use camino::Utf8PathBuf;
use serde::Serialize;
use tree_sitter::Parser;

use crate::client::graphql::{GraphQLParseError, parse_graphql};

/// Supported languages for client extraction.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub enum ExtractLanguage {
    TypeScript,
    Swift,
    Kotlin,
}

impl ExtractLanguage {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "ts" | "tsx" => Some(Self::TypeScript),
            "swift" => Some(Self::Swift),
            "kt" | "kts" => Some(Self::Kotlin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtractedDocument {
    pub content: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SkipReason {
    UnsupportedInterpolation,
    GraphQlSyntax(String),
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct ExtractResult {
    pub documents: Vec<ExtractedDocument>,
    pub skipped: Vec<(usize, SkipReason)>,
}

/// Extract GraphQL documents from a given source string, using language-specific parsing.
pub fn extract_documents(
    language: ExtractLanguage,
    source: &str,
    allowed_tags: &[&str],
) -> ExtractResult {
    match language {
        ExtractLanguage::TypeScript => extract_typescript_documents(source, allowed_tags),
        ExtractLanguage::Swift => extract_triple_quote_documents(source),
        ExtractLanguage::Kotlin => extract_triple_quote_documents(source),
    }
}

fn extract_typescript_documents(source: &str, allowed_tags: &[&str]) -> ExtractResult {
    let mut parser = Parser::new();
    let mut result = ExtractResult::default();
    if parser
        .set_language(&tree_sitter_typescript::language_tsx())
        .is_err()
    {
        return result;
    }
    let tree = match parser.parse(source, None) {
        Some(tree) => tree,
        None => return result,
    };
    let root = tree.root_node();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
        match node.kind() {
            "tagged_template" | "tagged_template_expression" | "template_string" => {
                if let Some(doc) = parse_tagged_template(source, &node, allowed_tags) {
                    match doc {
                        Ok(doc) => match parse_graphql(&doc.content) {
                            Ok(_) => result.documents.push(doc),
                            Err(GraphQLParseError::Syntax(msg)) => result
                                .skipped
                                .push((doc.line, SkipReason::GraphQlSyntax(msg))),
                        },
                        Err((line, reason)) => result.skipped.push((line, reason)),
                    }
                }
            }
            "call_expression" => {
                if let Some(doc) = parse_call_expression(source, &node, allowed_tags) {
                    match doc {
                        Ok(doc) => match parse_graphql(&doc.content) {
                            Ok(_) => result.documents.push(doc),
                            Err(GraphQLParseError::Syntax(msg)) => result
                                .skipped
                                .push((doc.line, SkipReason::GraphQlSyntax(msg))),
                        },
                        Err((line, reason)) => result.skipped.push((line, reason)),
                    }
                }
            }
            _ => {}
        }
    }
    result
}

fn parse_tagged_template(
    source: &str,
    node: &tree_sitter::Node<'_>,
    allowed_tags: &[&str],
) -> Option<Result<ExtractedDocument, (usize, SkipReason)>> {
    let tag_node = node.child(0)?;
    let tag_text = tag_node.utf8_text(source.as_bytes()).ok()?;
    let template_node = find_template_child(node, "template_string")?;
    extract_template_node(source, &tag_text, &template_node, allowed_tags)
}

fn parse_call_expression(
    source: &str,
    node: &tree_sitter::Node<'_>,
    allowed_tags: &[&str],
) -> Option<Result<ExtractedDocument, (usize, SkipReason)>> {
    let func_node = node
        .child_by_field_name("function")
        .or_else(|| node.child(0))?;
    let func_text = func_node.utf8_text(source.as_bytes()).ok()?;
    let template_node = find_template_child(node, "template_string")?;
    extract_template_node(source, &func_text, &template_node, allowed_tags)
}

fn find_template_child<'a>(
    node: &tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return Some(child);
        }
    }
    None
}

fn extract_template_node(
    source: &str,
    tag_text: &str,
    template_node: &tree_sitter::Node<'_>,
    allowed_tags: &[&str],
) -> Option<Result<ExtractedDocument, (usize, SkipReason)>> {
    if !allowed_tags.contains(&tag_text.trim()) {
        return None;
    }
    let template_text = template_node.utf8_text(source.as_bytes()).ok()?;
    let line = template_node.start_position().row + 1;
    if template_text.contains("${") {
        return Some(Err((line, SkipReason::UnsupportedInterpolation)));
    }
    let content = template_text.trim_matches('`').trim().to_string();

    Some(Ok(ExtractedDocument { content, line }))
}

fn extract_triple_quote_documents(source: &str) -> ExtractResult {
    let mut result = ExtractResult::default();
    let markers: Vec<_> = source.match_indices(r#""""#).collect();
    for pair in markers.chunks(2) {
        let [(start, _), (end, _)] = pair else {
            continue;
        };
        let start_idx = *start + 3;
        if start_idx > source.len() || *end <= start_idx {
            continue;
        }
        let body = &source[start_idx..*end];
        let line = source[..*start].chars().filter(|c| *c == '\n').count() + 1;
        match parse_graphql(body) {
            Ok(_) => result.documents.push(ExtractedDocument {
                content: body.trim().to_string(),
                line,
            }),
            Err(GraphQLParseError::Syntax(msg)) => {
                result.skipped.push((line, SkipReason::GraphQlSyntax(msg)))
            }
        }
    }
    result
}

#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct MaterializedFile {
    pub source: Utf8PathBuf,
    pub target: Utf8PathBuf,
    pub documents: usize,
}

#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct ExtractionSummary {
    pub out_dir: Utf8PathBuf,
    pub source_files_processed: usize,
    pub source_files_with_graphql: usize,
    pub documents_extracted: usize,
    pub documents_skipped: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_simple_typescript_gql() {
        let source = r#"
        import { gql } from "@apollo/client";
        const query = gql`
          query GetUser { user { id } }
        `;
        "#;

        let result = extract_documents(ExtractLanguage::TypeScript, source, &["gql"]);
        assert_eq!(result.documents.len(), 1);
        assert!(result.skipped.is_empty());
        assert!(result.documents[0].content.contains("query GetUser"));
    }

    #[test]
    fn skips_interpolated_typescript() {
        let source = r#"
        const query = gql`query User { user(id: ${id}) { id } }`;
        "#;
        let result = extract_documents(ExtractLanguage::TypeScript, source, &["gql"]);
        assert_eq!(result.documents.len(), 0);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].1, SkipReason::UnsupportedInterpolation);
    }

    #[test]
    fn extracts_swift_triple_quotes() {
        let source = r#"
        let query = """
        query GetUser { user { id } }
        """
        "#;

        let result = extract_documents(ExtractLanguage::Swift, source, &[]);
        assert_eq!(result.documents.len(), 1);
        assert!(result.documents[0].content.contains("query GetUser"));
    }

    #[test]
    fn reports_graphql_syntax_error() {
        let source = r#"
        let bad = """
        query {
        """
        "#;
        let result = extract_documents(ExtractLanguage::Swift, source, &[]);
        assert!(result.documents.is_empty());
        assert_eq!(result.skipped.len(), 1);
    }
}
