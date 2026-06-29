use std::fmt;

use apollo_compiler::{Node, ast};

pub(super) enum PrintableDefinition {
    Operation(Node<ast::OperationDefinition>),
    Fragment(Node<ast::FragmentDefinition>),
}

impl fmt::Display for PrintableDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrintableDefinition::Operation(op) => write!(f, "{}", op.serialize()),
            PrintableDefinition::Fragment(frag) => write!(f, "{}", frag.serialize()),
        }
    }
}

pub(super) fn print_document(definitions: &[PrintableDefinition]) -> String {
    definitions
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use apollo_compiler::{ast, parser::Parser as ApolloParser};
    use indoc::indoc;
    use speculoos::prelude::*;

    use super::*;

    fn parse_print(src: &str) -> String {
        let doc = ApolloParser::new().parse_ast(src, "test.graphql").unwrap();
        let defs: Vec<PrintableDefinition> = doc
            .definitions
            .into_iter()
            .filter_map(|d| match d {
                ast::Definition::OperationDefinition(op) => {
                    Some(PrintableDefinition::Operation(op))
                }
                ast::Definition::FragmentDefinition(frag) => {
                    Some(PrintableDefinition::Fragment(frag))
                }
                _ => None,
            })
            .collect();
        print_document(&defs)
    }

    #[test]
    fn prints_full_document_with_operation_and_fragment() {
        let src = indoc! {r#"
            query GetUser($id: ID!, $includeEmail: Boolean = false) {
              user(id: $id) {
                ...UserFields
                ... on Admin {
                  role
                }
              }
            }

            fragment UserFields on User {
              id
              name
              email @include(if: $includeEmail)
            }
        "#};
        let expected = indoc! {r#"
            query GetUser($id: ID!, $includeEmail: Boolean = false) {
              user(id: $id) {
                ...UserFields
                ... on Admin {
                  role
                }
              }
            }

            fragment UserFields on User {
              id
              name
              email @include(if: $includeEmail)
            }"#};
        assert_that!(parse_print(src)).is_equal_to(expected.to_string());
    }
}
