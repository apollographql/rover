#![allow(dead_code)]

use std::{collections::BTreeSet, sync::Arc};

use apollo_compiler::{Node, ast};

use super::{
    fragment_definition::FragmentDefinitionExt, selection::SelectionExt,
    selection_set::SelectionSetExt, variables::collect_variables_from_directives,
};

pub trait OperationDefinitionExt {
    /// Strips `@client` fields and directives, then injects `__typename` into every server-bound sub-selection.
    fn add_typenames(&mut self);
    /// Removes variable definitions not referenced in this operation or any of its associated fragments.
    fn prune_unused_variables(&mut self, fragments: &[(Node<ast::FragmentDefinition>, Arc<str>)]);
    fn collect_variables(&self) -> BTreeSet<String>;
}

impl OperationDefinitionExt for ast::OperationDefinition {
    fn add_typenames(&mut self) {
        self.selection_set.remove_client_selections();
        self.directives
            .0
            .retain(|directive| directive.name != "client");
        for selection in &mut self.selection_set {
            selection.add_typename();
        }
    }

    fn prune_unused_variables(&mut self, fragments: &[(Node<ast::FragmentDefinition>, Arc<str>)]) {
        let used: BTreeSet<String> = std::iter::once(self.collect_variables())
            .chain(fragments.iter().map(|(f, _)| f.collect_variables()))
            .fold(BTreeSet::new(), |mut acc, vars| {
                acc.extend(vars);
                acc
            });
        self.variables.retain(|v| used.contains(v.name.as_str()));
    }

    fn collect_variables(&self) -> BTreeSet<String> {
        let mut variables = BTreeSet::new();
        collect_variables_from_directives(&self.directives, &mut variables);
        self.selection_set.collect_variables(&mut variables);
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
    fn add_typenames_removes_client_directive_from_operation() {
        let mut op = parse_op("query Q @client { field }");
        op.make_mut().add_typenames();
        assert_that!(op.directives.has("client")).is_false();
    }

    #[test]
    fn add_typenames_removes_client_annotated_top_level_fields() {
        let mut op = parse_op("query Q { keep local @client }");
        op.make_mut().add_typenames();
        assert_that!(field_names(&op.selection_set)).does_not_contain("local");
        assert_that!(field_names(&op.selection_set)).contains("keep");
    }

    #[test]
    fn collect_variables_finds_all_variable_references() {
        let op = parse_op("query Q($a: ID!, $b: Int) { field(id: $a, limit: $b) }");
        let vars = op.collect_variables();
        assert_that!(vars.contains("a")).is_true();
        assert_that!(vars.contains("b")).is_true();
    }

    #[test]
    fn prune_unused_variables_removes_variable_not_in_operation_or_fragments() {
        let mut op = parse_op("query Q($used: ID!, $unused: ID!) { field(id: $used) }");
        op.make_mut().prune_unused_variables(&[]);
        let var_names: Vec<&str> = op.variables.iter().map(|v| v.name.as_str()).collect();
        assert_that!(var_names.contains(&"used")).is_true();
        assert_that!(var_names.contains(&"unused")).is_false();
    }

    #[test]
    fn prune_unused_variables_keeps_variable_used_only_in_fragment() {
        let mut op = parse_op("query Q($fragVar: ID!) { ...Frag }");
        let frag = parse_fragment("fragment Frag on T { field(id: $fragVar) }");
        let source = Arc::from("fragment Frag on T { field(id: $fragVar) }");
        op.make_mut().prune_unused_variables(&[(frag, source)]);
        assert_that!(op.variables.iter().map(|v| v.name.as_str()).any(|x| x == "fragVar")).is_true();
    }
}
