#![allow(dead_code)]

use std::collections::BTreeSet;

use apollo_compiler::ast;

use super::{selection_set::SelectionSetExt, variables::collect_variables_from_directives};

/// Extension methods for [`ast::FragmentDefinition`].
pub trait FragmentDefinitionExt {
    /// Returns all variable names referenced in the fragment's directives and selection set.
    fn collect_variables(&self) -> BTreeSet<String>;
}

impl FragmentDefinitionExt for ast::FragmentDefinition {
    fn collect_variables(&self) -> BTreeSet<String> {
        let mut variables = collect_variables_from_directives(&self.directives);
        variables.extend(self.selection_set.collect_variables());
        variables
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::{Node, parser::Parser as ApolloParser};
    use speculoos::prelude::*;

    use super::*;

    fn parse_fragment(src: &str) -> Node<ast::FragmentDefinition> {
        ApolloParser::new()
            .parse_ast(src, "test.graphql")
            .unwrap()
            .definitions
            .into_iter()
            .find_map(|d| {
                if let ast::Definition::FragmentDefinition(f) = d {
                    Some(f)
                } else {
                    None
                }
            })
            .unwrap()
    }

    #[test]
    fn collect_variables_finds_variables_in_field_arguments() {
        let frag = parse_fragment("fragment F on T { user(id: $userId) { name } }");
        let vars = frag.collect_variables();
        assert_that!(&vars).contains("userId".to_string());
    }

    #[test]
    fn collect_variables_finds_variables_in_directives() {
        let frag = parse_fragment("fragment F on T @include(if: $show) { field }");
        let vars = frag.collect_variables();
        assert_that!(&vars).contains("show".to_string());
    }
}
