#![allow(dead_code)]

use std::collections::BTreeSet;

use apollo_compiler::{Node, ast};

use super::{selection_set::SelectionSetExt, variables::collect_variables_from_directives};

/// Extension methods for [`ast::OperationDefinition`].
pub trait OperationDefinitionExt {
    /// Returns all variable names referenced in the operation's directives and selection set.
    fn collect_variables(&self) -> BTreeSet<String>;
}

impl OperationDefinitionExt for ast::OperationDefinition {
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

    fn parse_op(src: &str) -> Node<ast::OperationDefinition> {
        ApolloParser::new()
            .parse_ast(src, "test.graphql")
            .unwrap()
            .definitions
            .into_iter()
            .find_map(|d| {
                if let ast::Definition::OperationDefinition(op) = d {
                    Some(op)
                } else {
                    None
                }
            })
            .unwrap()
    }

    #[test]
    fn collect_variables_finds_all_variable_references() {
        let op = parse_op("query Q($a: ID!, $b: Int) { field(id: $a, limit: $b) }");
        let vars = op.collect_variables();
        assert_that!(vars.contains("a")).is_true();
        assert_that!(vars.contains("b")).is_true();
    }
}
