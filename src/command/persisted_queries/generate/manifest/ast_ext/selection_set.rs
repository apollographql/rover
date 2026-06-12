use std::collections::BTreeSet;

use apollo_compiler::{Node, ast};

use super::{
    selection::SelectionExt,
    variables::{collect_variables_from_arguments, collect_variables_from_directives},
};

pub trait SelectionSetExt {
    /// Collects named fragment spreads reachable from this set, excluding spreads under `@client`-annotated selections.
    fn collect_spreads(&self) -> BTreeSet<String>;
    fn collect_variables(&self, into: &mut BTreeSet<String>);
    fn remove_client_selections(&mut self);
    /// Like `add_typenames`, but only appends `__typename` when `append` is true.
    /// `@client` removal and recursion into sub-selections happen regardless of `append`.
    fn add_typenames_if(&mut self, append: bool);

    fn add_typenames(&mut self) {
        self.add_typenames_if(true);
    }
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

    fn collect_variables(&self, into: &mut BTreeSet<String>) {
        for selection in self {
            match selection {
                ast::Selection::Field(field) => {
                    collect_variables_from_arguments(&field.arguments, into);
                    collect_variables_from_directives(&field.directives, into);
                    field.selection_set.collect_variables(into);
                }
                ast::Selection::FragmentSpread(fs) => {
                    collect_variables_from_directives(&fs.directives, into);
                }
                ast::Selection::InlineFragment(inf) => {
                    collect_variables_from_directives(&inf.directives, into);
                    inf.selection_set.collect_variables(into);
                }
            }
        }
    }

    fn remove_client_selections(&mut self) {
        self.retain(|s| !s.has_directive("client"));
    }

    fn add_typenames_if(&mut self, append: bool) {
        self.remove_client_selections();
        for selection in self.iter_mut() {
            selection.add_typename();
        }
        if append && !self.iter().any(|s| s.is_typename_field()) {
            self.push(ast::Selection::Field(Node::new(ast::Field {
                alias: None,
                name: apollo_compiler::name!("__typename"),
                arguments: Vec::new(),
                directives: ast::DirectiveList::new(),
                selection_set: Vec::new(),
            })));
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
    fn collect_variables_finds_field_argument_variable() {
        let selections = parse_selections("query Q { user(id: $userId) { name } }");
        let mut vars = BTreeSet::new();
        selections.collect_variables(&mut vars);
        assert_that!(vars.contains("userId")).is_true();
    }

    #[test]
    fn collect_variables_finds_directive_argument_variable() {
        let selections = parse_selections("query Q { field @include(if: $show) }");
        let mut vars = BTreeSet::new();
        selections.collect_variables(&mut vars);
        assert_that!(vars.contains("show")).is_true();
    }

    #[test]
    fn collect_variables_finds_nested_field_variable() {
        let selections = parse_selections("query Q { parent { child(x: $val) } }");
        let mut vars = BTreeSet::new();
        selections.collect_variables(&mut vars);
        assert_that!(vars.contains("val")).is_true();
    }

    #[test]
    fn collect_variables_finds_variable_in_list_value() {
        let selections = parse_selections("query Q { field(ids: [$a, $b]) }");
        let mut vars = BTreeSet::new();
        selections.collect_variables(&mut vars);
        assert_that!(vars.contains("a")).is_true();
        assert_that!(vars.contains("b")).is_true();
    }

    #[test]
    fn collect_variables_finds_variable_in_object_value() {
        let selections = parse_selections("query Q { field(input: {id: $x}) }");
        let mut vars = BTreeSet::new();
        selections.collect_variables(&mut vars);
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

    #[test]
    fn add_typenames_appends_typename_to_non_empty_selection_set() {
        let mut selections = parse_selections("query Q { id }");
        selections.add_typenames();
        assert_that!(field_names(&selections)).contains("__typename");
    }

    #[test]
    fn add_typenames_does_not_add_duplicate_typename() {
        let mut selections = parse_selections("query Q { id __typename }");
        selections.add_typenames();
        let count = field_names(&selections)
            .iter()
            .filter(|&&n| n == "__typename")
            .count();
        assert_that!(count).is_equal_to(1);
    }

    #[test]
    fn add_typenames_recurses_into_nested_fields() {
        let mut selections = parse_selections("query Q { parent { child } }");
        selections.add_typenames();
        if let ast::Selection::Field(parent) = &selections[0] {
            assert_that!(field_names(&parent.selection_set)).contains("__typename");
        } else {
            panic!("expected field");
        }
    }

    #[test]
    fn add_typenames_if_false_does_not_append_typename() {
        let mut selections = parse_selections("query Q { id }");
        selections.add_typenames_if(false);
        assert_that!(field_names(&selections)).does_not_contain("__typename");
    }

    #[test]
    fn add_typenames_skips_typename_on_export_annotated_fields() {
        let mut selections = parse_selections(r#"query Q { data @export(as: "data") { id } }"#);
        selections.add_typenames();
        if let ast::Selection::Field(data) = &selections[0] {
            assert_that!(field_names(&data.selection_set)).does_not_contain("__typename");
        } else {
            panic!("expected field");
        }
    }
}
