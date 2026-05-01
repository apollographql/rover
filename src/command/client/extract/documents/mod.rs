pub mod triple_quote;
pub mod typescript;

use serde::Serialize;
pub use triple_quote::ExtractTripleQuoteDocuments;
pub use typescript::ExtractTypescriptDocuments;

use super::{ExtractResult, language::ExtractLanguage};

pub trait ExtractDocuments {
    fn extract_documents(&self, source: &str) -> ExtractResult;
}

#[derive(thiserror::Error, Debug, Serialize)]
pub enum SkipReason {
    #[error("contains a template interpolation (${{...}}); only static strings can be extracted")]
    UnsupportedInterpolation,
    #[error("GraphQL syntax error: {0}")]
    GraphQlSyntax(String),
    #[error("unclosed triple-quote block; no matching closing \"\"\"")]
    UnclosedTripleQuote,
}

#[derive(Debug, Serialize)]
pub struct SkippedDocument {
    pub line: usize,
    pub reason: SkipReason,
}

impl ExtractDocuments for ExtractLanguage {
    fn extract_documents(&self, source: &str) -> ExtractResult {
        match self {
            Self::TypeScript => ExtractTypescriptDocuments {
                allowed_tags: vec!["gql".to_string(), "graphql".to_string()],
            }
            .extract_documents(source),
            Self::Swift | Self::Kotlin => ExtractTripleQuoteDocuments.extract_documents(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn extracts_simple_typescript_gql() {
        let source = r#"
        import { gql } from "@apollo/client";
        const query = gql`
          query GetUser { user { id } }
        `;
        "#;

        let result = ExtractTypescriptDocuments {
            allowed_tags: vec!["gql".into()],
        }
        .extract_documents(source);

        assert_that!(&result.documents).has_length(1);
        assert_that!(&result.skipped).is_empty();
        assert_that!(&result.documents[0].content).contains("query GetUser");
    }

    #[test]
    fn skips_interpolated_typescript() {
        // `${id}` is a JavaScript template literal interpolation — a runtime value injected into
        // the string. GraphQL documents must be static so they can be extracted at build time;
        // we can't know the interpolated value ahead of execution, so we skip these templates
        // rather than extracting a broken or incomplete document.
        let source = r#"
        const query = gql`query User { user(id: ${id}) { id } }`;
        "#;
        let result = ExtractTypescriptDocuments {
            allowed_tags: vec!["gql".into()],
        }
        .extract_documents(source);

        assert_that!(&result.documents).is_empty();
        assert_that!(&result.skipped).has_length(1);
        assert_that!(&result.skipped[0].reason)
            .matches(|r| matches!(r, SkipReason::UnsupportedInterpolation));
    }

    #[rstest]
    #[case::ts_gql_tag(
        ExtractLanguage::TypeScript,
        "const q = gql`query GetUser { user { id } }`;"
    )]
    #[case::ts_graphql_tag(
        ExtractLanguage::TypeScript,
        "const q = graphql`query GetUser { user { id } }`;"
    )]
    #[case::swift(
        ExtractLanguage::Swift,
        r#"let q = """
query GetUser { user { id } }
""""#
    )]
    #[case::kotlin(
        ExtractLanguage::Kotlin,
        r#"val q = """
query GetUser { user { id } }
""""#
    )]
    fn language_dispatch_extracts_document(
        #[case] language: ExtractLanguage,
        #[case] source: &str,
    ) {
        let result = language.extract_documents(source);
        assert_that!(&result.documents).has_length(1);
        assert_that!(&result.documents[0].content).contains("query GetUser");
    }

    #[test]
    fn extracts_swift_triple_quotes() {
        let source = r#"
        let query = """
        query GetUser { user { id } }
        """
        "#;

        let result = ExtractTripleQuoteDocuments.extract_documents(source);

        assert_that!(&result.documents).has_length(1);
        assert_that!(&result.documents[0].content).contains("query GetUser");
    }

    #[test]
    fn reports_graphql_syntax_error_in_triple_quote() {
        let source = r#"
        let bad = """
        query {
        """
        "#;
        let result = ExtractTripleQuoteDocuments.extract_documents(source);

        assert_that!(&result.documents).is_empty();
        assert_that!(&result.skipped).has_length(1);
        assert_that!(&result.skipped[0].reason)
            .matches(|r| matches!(r, SkipReason::GraphQlSyntax(msg) if !msg.is_empty()));
    }
}
