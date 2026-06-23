#![allow(dead_code)]

use apollo_compiler::ast;

/// Extension methods for [`ast::Selection`].
pub trait SelectionExt {
    /// Returns `true` if this selection carries a directive with the given `name`.
    fn has_directive(&self, name: &str) -> bool;
}

impl SelectionExt for ast::Selection {
    fn has_directive(&self, name: &str) -> bool {
        match self {
            ast::Selection::Field(field) => field.directives.has(name),
            ast::Selection::FragmentSpread(fs) => fs.directives.has(name),
            ast::Selection::InlineFragment(inf) => inf.directives.has(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::parser::Parser as ApolloParser;
    use speculoos::prelude::*;

    use super::*;

    fn parse_selections(src: &str) -> Vec<ast::Selection> {
        ApolloParser::new()
            .parse_ast(src, "test.graphql")
            .unwrap()
            .definitions
            .into_iter()
            .find_map(|d| {
                if let ast::Definition::OperationDefinition(op) = d {
                    Some(op.selection_set.clone())
                } else {
                    None
                }
            })
            .unwrap()
    }

    #[test]
    fn has_directive_is_true_for_matching_directive_on_field() {
        let selections = parse_selections("query Q { field @client }");
        assert_that!(selections[0].has_directive("client")).is_true();
    }

    #[test]
    fn has_directive_is_false_for_absent_directive_on_field() {
        let selections = parse_selections("query Q { field @other }");
        assert_that!(selections[0].has_directive("client")).is_false();
    }

    #[test]
    fn has_directive_is_true_for_directive_on_fragment_spread() {
        let selections = parse_selections("query Q { ...Frag @client }");
        assert_that!(selections[0].has_directive("client")).is_true();
    }

    #[test]
    fn has_directive_is_true_for_directive_on_inline_fragment() {
        let selections = parse_selections("query Q { ... @client { field } }");
        assert_that!(selections[0].has_directive("client")).is_true();
    }
}
