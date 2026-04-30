use tree_sitter::Parser;

use super::{
    super::{
        ExtractedDocument,
        graphql::{GraphQLParseError, parse_graphql},
    },
    ExtractDocuments, ExtractResult,
};
use crate::command::client::extract::documents::{SkipReason, SkippedDocument};

pub struct ExtractTypescriptDocuments {
    pub allowed_tags: Vec<String>,
}

impl ExtractDocuments for ExtractTypescriptDocuments {
    fn extract_documents(&self, source: &str) -> ExtractResult {
        let allowed_tags: Vec<&str> = self.allowed_tags.iter().map(String::as_str).collect();
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
                    if let Some(doc) = parse_tagged_template(source, &node, &allowed_tags) {
                        match doc {
                            Ok(doc) => match parse_graphql(&doc.content) {
                                Ok(_) => result.documents.push(doc),
                                Err(GraphQLParseError::Syntax(msg)) => {
                                    result.skipped.push(SkippedDocument {
                                        line: doc.line,
                                        reason: SkipReason::GraphQlSyntax(msg),
                                    })
                                }
                            },
                            Err(skipped) => result.skipped.push(skipped),
                        }
                    }
                }
                "call_expression" => {
                    if let Some(doc) = parse_call_expression(source, &node, &allowed_tags) {
                        match doc {
                            Ok(doc) => match parse_graphql(&doc.content) {
                                Ok(_) => result.documents.push(doc),
                                Err(GraphQLParseError::Syntax(msg)) => {
                                    result.skipped.push(SkippedDocument {
                                        line: doc.line,
                                        reason: SkipReason::GraphQlSyntax(msg),
                                    })
                                }
                            },
                            Err(skipped) => result.skipped.push(skipped),
                        }
                    }
                }
                _ => {}
            }
        }
        result
    }
}

fn parse_tagged_template(
    source: &str,
    node: &tree_sitter::Node<'_>,
    allowed_tags: &[&str],
) -> Option<Result<ExtractedDocument, SkippedDocument>> {
    let tag_node = node.child(0)?;
    let tag_text = tag_node.utf8_text(source.as_bytes()).ok()?;
    let template_node = find_template_child(node, "template_string")?;
    extract_template_node(source, tag_text, &template_node, allowed_tags)
}

fn parse_call_expression(
    source: &str,
    node: &tree_sitter::Node<'_>,
    allowed_tags: &[&str],
) -> Option<Result<ExtractedDocument, SkippedDocument>> {
    let func_node = node
        .child_by_field_name("function")
        .or_else(|| node.child(0))?;
    let func_text = func_node.utf8_text(source.as_bytes()).ok()?;
    let template_node = find_template_child(node, "template_string")?;
    extract_template_node(source, func_text, &template_node, allowed_tags)
}

fn find_template_child<'a>(
    node: &tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn extract_template_node(
    source: &str,
    tag_text: &str,
    template_node: &tree_sitter::Node<'_>,
    allowed_tags: &[&str],
) -> Option<Result<ExtractedDocument, SkippedDocument>> {
    if !allowed_tags.contains(&tag_text.trim()) {
        return None;
    }
    let template_text = template_node.utf8_text(source.as_bytes()).ok()?;
    let line = template_node.start_position().row + 1;
    if template_text.contains("${") {
        return Some(Err(SkippedDocument {
            line,
            reason: SkipReason::UnsupportedInterpolation,
        }));
    }
    let content = template_text.trim_matches('`').trim().to_string();

    Some(Ok(ExtractedDocument { content, line }))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    fn extractor(tags: &[&str]) -> ExtractTypescriptDocuments {
        ExtractTypescriptDocuments {
            allowed_tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[rstest]
    #[case::gql_tag("gql", "const q = gql`query GetUser { user { id } }`;")]
    #[case::graphql_tag("graphql", "const q = graphql`query GetUser { user { id } }`;")]
    fn extracts_tagged_template_with_allowed_tag(#[case] tag: &str, #[case] source: &str) {
        let result = extractor(&[tag]).extract_documents(source);

        assert_that!(&result.documents).has_length(1);
        assert_that!(&result.skipped).is_empty();
        assert_that!(&result.documents[0].content).contains("query GetUser");
    }

    #[test]
    fn extracts_multiple_documents_from_one_file() {
        let source = r#"
        const q1 = gql`query A { a { id } }`;
        const q2 = gql`query B { b { id } }`;
        "#;
        let result = extractor(&["gql"]).extract_documents(source);

        assert_that!(&result.documents).has_length(2);
        let contents: Vec<&str> = result
            .documents
            .iter()
            .map(|d| d.content.as_str())
            .collect();
        assert_that!(contents.iter().any(|c| c.contains("query A"))).is_true();
        assert_that!(contents.iter().any(|c| c.contains("query B"))).is_true();
    }

    #[test]
    fn skips_template_with_invalid_graphql_syntax() {
        let source = "const q = gql`query { unclosed {`;";
        let result = extractor(&["gql"]).extract_documents(source);

        assert_that!(&result.documents).is_empty();
        assert_that!(&result.skipped).has_length(1);
        assert_that!(&result.skipped[0].reason)
            .matches(|r| matches!(r, SkipReason::GraphQlSyntax(msg) if !msg.is_empty()));
    }

    #[test]
    fn ignores_tags_not_in_allowed_list() {
        let source = "const q = myTag`query GetUser { user { id } }`;";
        let result = extractor(&["gql"]).extract_documents(source);

        assert_that!(&result.documents).is_empty();
        assert_that!(&result.skipped).is_empty();
    }
}
