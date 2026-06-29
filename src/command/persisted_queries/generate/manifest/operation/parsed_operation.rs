use std::collections::{BTreeMap, BTreeSet};

use apollo_compiler::{Node, ast};
use camino::Utf8PathBuf;

use super::parsed_fragment::ParsedFragment;
use crate::command::persisted_queries::generate::manifest::{
    ast_ext::{FragmentDefinitionExt, OperationDefinitionExt, SelectionSetExt},
    error::GenerateError,
    printer::{PrintableDefinition, print_document},
};

#[derive(Debug, Clone)]
pub(super) struct ParsedOperation {
    pub(super) file: Utf8PathBuf,
    pub(super) operation: Node<ast::OperationDefinition>,
    pub(super) direct_fragment_spreads: BTreeSet<String>,
}

impl ParsedOperation {
    pub(super) fn reachable_fragment_names(
        &self,
        name: &str,
        all_fragments: &BTreeMap<String, ParsedFragment>,
    ) -> Result<BTreeSet<String>, GenerateError> {
        let mut reachable = BTreeSet::new();
        let mut queue: Vec<&str> = self
            .direct_fragment_spreads
            .iter()
            .map(String::as_str)
            .collect();

        while let Some(fragment_name) = queue.pop() {
            if !reachable.insert(fragment_name.to_string()) {
                continue;
            }
            let fragment =
                all_fragments
                    .get(fragment_name)
                    .ok_or_else(|| GenerateError::MissingFragment {
                        operation_name: name.to_string(),
                        fragment_name: fragment_name.to_string(),
                    })?;
            queue.extend(fragment.direct_fragment_spreads.iter().map(String::as_str));
        }

        Ok(reachable)
    }

    pub(super) fn body(
        &self,
        name: &str,
        all_fragments: &BTreeMap<String, ParsedFragment>,
    ) -> Result<String, GenerateError> {
        let reachable = self.reachable_fragment_names(name, all_fragments)?;
        let mut operation_node = self.operation.clone();
        {
            let op_mut = operation_node.make_mut();
            op_mut.selection_set.remove_client_selections();
            op_mut.directives.0.retain(|d| d.name != "client");
        }

        let fragment_definitions: Vec<Node<ast::FragmentDefinition>> = reachable
            .iter()
            .map(|fragment_name| -> Result<_, GenerateError> {
                let fragment = all_fragments.get(fragment_name).ok_or_else(|| {
                    GenerateError::MissingFragment {
                        operation_name: name.to_string(),
                        fragment_name: fragment_name.to_string(),
                    }
                })?;
                let mut fragment_node = fragment.fragment.clone();
                let fragment_definition = fragment_node.make_mut();
                fragment_definition
                    .directives
                    .0
                    .retain(|directive| directive.name != "client");
                fragment_definition.selection_set.remove_client_selections();
                Ok(fragment_node)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let op = operation_node.make_mut();
        let used: BTreeSet<String> = std::iter::once(op.collect_variables())
            .chain(fragment_definitions.iter().map(|f| f.collect_variables()))
            .fold(BTreeSet::new(), |mut acc, vars| {
                acc.extend(vars);
                acc
            });
        op.variables.retain(|v| used.contains(v.name.as_str()));

        let definitions = std::iter::once(PrintableDefinition::Operation(operation_node))
            .chain(
                fragment_definitions
                    .into_iter()
                    .map(PrintableDefinition::Fragment),
            )
            .collect::<Vec<_>>();

        Ok(print_document(&definitions))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use apollo_compiler::{ast, parser::Parser as ApolloParser};
    use camino::Utf8PathBuf;
    use speculoos::prelude::*;

    use super::ParsedOperation;
    use crate::command::persisted_queries::generate::manifest::{
        ast_ext::SelectionSetExt, operation::parsed_fragment::ParsedFragment,
    };

    fn parse_doc(
        src: &str,
    ) -> (
        BTreeMap<String, ParsedOperation>,
        BTreeMap<String, ParsedFragment>,
    ) {
        let doc = ApolloParser::new().parse_ast(src, "test.graphql").unwrap();
        let mut operations = BTreeMap::new();
        let mut fragments = BTreeMap::new();
        for def in doc.definitions {
            match def {
                ast::Definition::OperationDefinition(op) => {
                    let name = op.name.as_ref().unwrap().to_string();
                    let spreads = op.selection_set.collect_spreads();
                    operations.insert(
                        name,
                        ParsedOperation {
                            file: Utf8PathBuf::from("test.graphql"),
                            direct_fragment_spreads: spreads,
                            operation: op,
                        },
                    );
                }
                ast::Definition::FragmentDefinition(frag) => {
                    let name = frag.name.to_string();
                    let spreads = frag.selection_set.collect_spreads();
                    fragments.insert(
                        name,
                        ParsedFragment {
                            file: Utf8PathBuf::from("test.graphql"),
                            direct_fragment_spreads: spreads,
                            fragment: frag,
                        },
                    );
                }
                _ => {}
            }
        }
        (operations, fragments)
    }

    #[test]
    fn missing_fragment_reference_returns_error() {
        let (ops, _) = parse_doc("query GetUser { ...UserFields }");
        let op = ops.get("GetUser").unwrap();
        let result = op.reachable_fragment_names("GetUser", &BTreeMap::new());
        assert_that!(result).is_err();
        assert_that!(result.unwrap_err().to_string()).contains("UserFields");
    }

    #[test]
    fn reachable_fragments_are_sorted_by_name_and_transitive() {
        let (ops, frags) = parse_doc(indoc::indoc! {"
            fragment Zed on Product { z }
            fragment Alpha on Product { a ...Zed }
            query GetProduct { product { ...Alpha } }
        "});
        let op = ops.get("GetProduct").unwrap();
        let body = op.body("GetProduct", &frags).unwrap();
        assert_that!(body.as_str()).is_equal_to(indoc::indoc! {"
            query GetProduct {
              product {
                ...Alpha
              }
            }

            fragment Alpha on Product {
              a
              ...Zed
            }

            fragment Zed on Product {
              z
            }"});
    }

    #[test]
    fn client_directive_selections_match_default_typescript_transform() {
        let (ops, frags) = parse_doc(indoc::indoc! {"
            fragment LocalFields on CurrentUser {
              temporary @client
            }

            query CurrentUserQuery {
              isLoggedIn @client
              currentUser {
                id
                ...LocalFields @client
              }
            }
        "});
        let op = ops.get("CurrentUserQuery").unwrap();
        let body = op.body("CurrentUserQuery", &frags).unwrap();
        assert_that!(body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery {
              currentUser {
                id
              }
            }"});
    }

    #[test]
    fn client_directive_removal_removes_all_client_fields() {
        let (ops, frags) = parse_doc(indoc::indoc! {"
            query CurrentUserQuery {
              currentUser {
                localOnly @client
              }
            }
        "});
        let op = ops.get("CurrentUserQuery").unwrap();
        let body = op.body("CurrentUserQuery", &frags).unwrap();
        assert_that!(body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery {
              currentUser
            }"});
    }

    #[test]
    fn client_directive_removal_prunes_now_unused_variables() {
        let (ops, frags) = parse_doc(indoc::indoc! {"
            query CurrentUserQuery($localId: ID!, $userId: ID!) {
              isLoggedIn(id: $localId) @client
              currentUser(id: $userId) {
                id
              }
            }
        "});
        let op = ops.get("CurrentUserQuery").unwrap();
        let body = op.body("CurrentUserQuery", &frags).unwrap();
        assert_that!(body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery($userId: ID!) {
              currentUser(id: $userId) {
                id
              }
            }"});
    }

    #[test]
    fn variable_pruning_keeps_fragment_and_directive_variables() {
        let (ops, frags) = parse_doc(indoc::indoc! {"
            query CurrentUserQuery($userId: ID!, $includeFriends: Boolean!, $localId: ID!) {
              localUser(id: $localId) @client
              currentUser(id: $userId) {
                ...UserFields @include(if: $includeFriends)
              }
            }

            fragment UserFields on User {
              friends(first: $userId) {
                id
              }
            }
        "});
        let op = ops.get("CurrentUserQuery").unwrap();
        let body = op.body("CurrentUserQuery", &frags).unwrap();
        assert_that!(body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery($userId: ID!, $includeFriends: Boolean!) {
              currentUser(id: $userId) {
                ...UserFields @include(if: $includeFriends)
              }
            }

            fragment UserFields on User {
              friends(first: $userId) {
                id
              }
            }"});
    }
}
