use apollo_compiler::{Node, ast};

use super::selection_set::SelectionSetExt;

pub trait SelectionExt {
    fn has_directive(&self, name: &str) -> bool;
    fn is_typename_field(&self) -> bool;
    /// Injects `__typename` into sub-selections; skips fields annotated with `@export` (local-state transfers have no server schema).
    fn add_typename(&mut self);
}

impl SelectionExt for ast::Selection {
    fn has_directive(&self, name: &str) -> bool {
        match self {
            ast::Selection::Field(field) => field.directives.has(name),
            ast::Selection::FragmentSpread(fs) => fs.directives.has(name),
            ast::Selection::InlineFragment(inf) => inf.directives.has(name),
        }
    }

    fn is_typename_field(&self) -> bool {
        matches!(self, ast::Selection::Field(field) if field.name == "__typename")
    }

    fn add_typename(&mut self) {
        match self {
            ast::Selection::Field(field) => {
                let field = field.make_mut();
                if !field.selection_set.is_empty() {
                    let append = !field.directives.has("export");
                    field.selection_set.add_typenames_if(append);
                }
            }
            ast::Selection::InlineFragment(inf) => {
                inf.make_mut().selection_set.add_typenames();
            }
            ast::Selection::FragmentSpread(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::{Node, parser::Parser as ApolloParser};
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

    #[test]
    fn is_typename_field_is_true_only_for_dunder_typename_field() {
        let typename = ast::Selection::Field(Node::new(ast::Field {
            alias: None,
            name: apollo_compiler::name!("__typename"),
            arguments: vec![],
            directives: ast::DirectiveList::new(),
            selection_set: vec![],
        }));
        assert_that!(typename.is_typename_field()).is_true();

        let other = ast::Selection::Field(Node::new(ast::Field {
            alias: None,
            name: apollo_compiler::name!("id"),
            arguments: vec![],
            directives: ast::DirectiveList::new(),
            selection_set: vec![],
        }));
        assert_that!(other.is_typename_field()).is_false();
    }

    #[test]
    fn is_typename_field_is_false_for_fragment_spread() {
        let selections = parse_selections("query Q { ...Frag }");
        assert_that!(selections[0].is_typename_field()).is_false();
    }
}
