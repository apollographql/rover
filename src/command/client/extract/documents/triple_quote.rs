use super::{
    super::{
        graphql::{GraphQLParseError, parse_graphql},
        types::ExtractedDocument,
    },
    ExtractDocuments, ExtractResult, SkipReason, SkippedDocument,
};

pub struct ExtractTripleQuoteDocuments;

impl ExtractDocuments for ExtractTripleQuoteDocuments {
    fn extract_documents(&self, source: &str) -> ExtractResult {
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
                Err(GraphQLParseError::Syntax(msg)) => result.skipped.push(SkippedDocument {
                    line,
                    reason: SkipReason::GraphQlSyntax(msg),
                }),
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn extracts_multiple_blocks_from_one_file() {
        let source = r#"
        let q1 = """
        query A { a { id } }
        """
        let q2 = """
        query B { b { id } }
        """
        "#;
        let result = ExtractTripleQuoteDocuments.extract_documents(source);

        assert_that!(&result.documents).has_length(2);
        assert_that!(&result.skipped).is_empty();
        let contents: Vec<&str> = result
            .documents
            .iter()
            .map(|d| d.content.as_str())
            .collect();
        assert_that!(contents.iter().any(|c| c.contains("query A"))).is_true();
        assert_that!(contents.iter().any(|c| c.contains("query B"))).is_true();
    }

    #[test]
    fn unpaired_opening_marker_produces_no_output() {
        let source = r#"let q = """
        query A { a { id } }"#;
        let result = ExtractTripleQuoteDocuments.extract_documents(source);

        assert_that!(&result.documents).is_empty();
        assert_that!(&result.skipped).is_empty();
    }

    #[test]
    fn syntax_error_in_one_block_skips_that_block_only() {
        let source = r#"
        let bad = """
        query {
        """
        let good = """
        query A { a { id } }
        """
        "#;
        let result = ExtractTripleQuoteDocuments.extract_documents(source);

        assert_that!(&result.documents).has_length(1);
        assert_that!(&result.documents[0].content).contains("query A");
        assert_that!(&result.skipped).has_length(1);
        assert_that!(&result.skipped[0].reason)
            .matches(|r| matches!(r, SkipReason::GraphQlSyntax(msg) if !msg.is_empty()));
    }
}
