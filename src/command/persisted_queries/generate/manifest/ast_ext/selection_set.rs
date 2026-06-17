#![allow(dead_code)]

use std::collections::BTreeSet;

use apollo_compiler::ast;

use super::{
    selection::SelectionExt,
    variables::{collect_variables_from_arguments, collect_variables_from_directives},
};

/// Extension methods for a selection set (`Vec<ast::Selection>`).
pub trait SelectionSetExt {
    /// Collects named fragment spreads reachable from this set, excluding spreads under `@client`-annotated selections.
    fn collect_spreads(&self) -> BTreeSet<String>;
    /// Returns all variable names referenced anywhere in this selection set.
    /// Unlike `collect_spreads`, this intentionally includes variables inside
    /// `@client`-annotated selections: those variables are still declared on the
    /// operation and must be counted as used so they are not incorrectly pruned.
    fn collect_variables(&self) -> BTreeSet<String>;
    /// Recursively removes all selections annotated with `@client`.
    fn remove_client_selections(&mut self);
}

impl SelectionSetExt for Vec<ast::Selection> {
    fn collect_spreads(&self) -> BTreeSet<String> {
        self.iter()
            .filter(|s| !s.has_directive("client"))
            .flat_map(|s| -> Box<dyn Iterator<Item = String>> {
                match s {
                    ast::Selection::FragmentSpread(fs) => {
                        Box::new(std::iter::once(fs.fragment_name.to_string()))
                    }
                    ast::Selection::Field(field) => {
                        Box::new(field.selection_set.collect_spreads().into_iter())
                    }
                    ast::Selection::InlineFragment(inf) => {
                        Box::new(inf.selection_set.collect_spreads().into_iter())
                    }
                }
            })
            .collect()
    }

    fn collect_variables(&self) -> BTreeSet<String> {
        let mut variables = BTreeSet::new();
        for selection in self {
            match selection {
                ast::Selection::Field(field) => {
                    variables.extend(collect_variables_from_arguments(&field.arguments));
                    variables.extend(collect_variables_from_directives(&field.directives));
                    variables.extend(field.selection_set.collect_variables());
                }
                ast::Selection::FragmentSpread(fs) => {
                    variables.extend(collect_variables_from_directives(&fs.directives));
                }
                ast::Selection::InlineFragment(inf) => {
                    variables.extend(collect_variables_from_directives(&inf.directives));
                    variables.extend(inf.selection_set.collect_variables());
                }
            }
        }
        variables
    }

    fn remove_client_selections(&mut self) {
        self.retain(|s| !s.has_directive("client"));
        for selection in self.iter_mut() {
            match selection {
                ast::Selection::Field(field) => {
                    field.make_mut().selection_set.remove_client_selections();
                }
                ast::Selection::InlineFragment(inf) => {
                    inf.make_mut().selection_set.remove_client_selections();
                }
                ast::Selection::FragmentSpread(_) => {}
            }
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

    fn field_names(selections: &[ast::Selection]) -> Vec<&str> {
        selections
            .iter()
            .filter_map(|s| {
                if let ast::Selection::Field(f) = s {
                    Some(f.name.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn collect_spreads_finds_direct_spread() {
        let selections = parse_selections("query Q { ...MyFrag }");
        assert_that!(selections.collect_spreads().contains("MyFrag")).is_true();
    }

    #[test]
    fn collect_spreads_finds_spread_nested_inside_field() {
        let selections = parse_selections("query Q { parent { ...Inner } }");
        assert_that!(selections.collect_spreads().contains("Inner")).is_true();
    }

    #[test]
    fn collect_spreads_finds_spread_inside_inline_fragment() {
        let selections = parse_selections("query Q { ... on T { ...Inner } }");
        assert_that!(selections.collect_spreads().contains("Inner")).is_true();
    }

    #[test]
    fn collect_spreads_skips_client_annotated_spread() {
        let selections = parse_selections("query Q { ...LocalFrag @client }");
        assert_that!(selections.collect_spreads().contains("LocalFrag")).is_false();
    }

    #[test]
    fn collect_spreads_skips_spread_inside_client_annotated_inline_fragment() {
        let selections = parse_selections("query Q { ... @client { ...Frag } }");
        assert_that!(selections.collect_spreads().contains("Frag")).is_false();
    }

    #[test]
    fn collect_variables_finds_field_argument_variable() {
        let selections = parse_selections("query Q { user(id: $userId) { name } }");
        let vars = selections.collect_variables();
        assert_that!(vars.contains("userId")).is_true();
    }

    #[test]
    fn collect_variables_finds_directive_argument_variable() {
        let selections = parse_selections("query Q { field @include(if: $show) }");
        let vars = selections.collect_variables();
        assert_that!(vars.contains("show")).is_true();
    }

    #[test]
    fn collect_variables_finds_nested_field_variable() {
        let selections = parse_selections("query Q { parent { child(x: $val) } }");
        let vars = selections.collect_variables();
        assert_that!(vars.contains("val")).is_true();
    }

    #[test]
    fn collect_variables_finds_variable_in_list_value() {
        let selections = parse_selections("query Q { field(ids: [$a, $b]) }");
        let vars = selections.collect_variables();
        assert_that!(vars.contains("a")).is_true();
        assert_that!(vars.contains("b")).is_true();
    }

    #[test]
    fn collect_variables_finds_variable_in_object_value() {
        let selections = parse_selections("query Q { field(input: {id: $x}) }");
        let vars = selections.collect_variables();
        assert_that!(vars.contains("x")).is_true();
    }

    #[test]
    fn remove_client_selections_removes_fields_with_client_directive() {
        let mut selections = parse_selections("query Q { keep local @client }");
        selections.remove_client_selections();
        assert_that!(field_names(&selections)).is_equal_to(vec!["keep"]);
    }

    #[test]
    fn remove_client_selections_keeps_fields_without_directive() {
        let mut selections = parse_selections("query Q { a b c }");
        selections.remove_client_selections();
        assert_that!(field_names(&selections)).is_equal_to(vec!["a", "b", "c"]);
    }
}
