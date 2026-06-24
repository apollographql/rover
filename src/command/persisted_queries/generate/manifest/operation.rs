use std::collections::{BTreeMap, BTreeSet, HashMap};

use apollo_compiler::{Node, ast, parser::Parser as ApolloParser};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use rover_std::Fs;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    ast_ext::{FragmentDefinitionExt, OperationDefinitionExt, SelectionSetExt},
    error::{GenerateError, ParseFailure},
    printer::{PrintableDefinition, print_document},
};
use crate::RoverResult;

#[derive(Debug, Clone)]
struct ParsedOperation {
    file: Utf8PathBuf,
    operation: Node<ast::OperationDefinition>,
    direct_fragment_spreads: BTreeSet<String>,
}

impl ParsedOperation {
    fn reachable_fragment_names(
        &self,
        name: &str,
        all_fragments: &BTreeMap<String, ParsedFragment>,
    ) -> RoverResult<BTreeSet<String>> {
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

    fn body(
        &self,
        name: &str,
        all_fragments: &BTreeMap<String, ParsedFragment>,
    ) -> RoverResult<String> {
        let reachable = self.reachable_fragment_names(name, all_fragments)?;
        let mut operation_node = self.operation.clone();
        {
            let op_mut = operation_node.make_mut();
            op_mut.selection_set.remove_client_selections();
            op_mut.directives.0.retain(|d| d.name != "client");
        }

        let fragment_definitions: Vec<Node<ast::FragmentDefinition>> = reachable
            .iter()
            .map(|fragment_name| {
                let fragment = all_fragments
                    .get(fragment_name)
                    .expect("reachable fragments are validated before returning");
                let mut fragment_node = fragment.fragment.clone();
                let fragment_definition = fragment_node.make_mut();
                fragment_definition
                    .directives
                    .0
                    .retain(|directive| directive.name != "client");
                fragment_definition.selection_set.remove_client_selections();
                fragment_node
            })
            .collect();

        let op = operation_node.make_mut();
        let used: BTreeSet<String> = std::iter::once(op.collect_variables())
            .chain(fragment_definitions.iter().map(|f| f.collect_variables()))
            .fold(BTreeSet::new(), |mut acc, vars| {
                acc.extend(vars);
                acc
            });
        op.variables.retain(|v| used.contains(v.name.as_str()));

        let definitions = std::iter::once(PrintableDefinition::Operation(operation_node))
            .chain(fragment_definitions.into_iter().map(PrintableDefinition::Fragment))
            .collect::<Vec<_>>();

        Ok(print_document(&definitions))
    }
}

#[derive(Debug, Clone)]
struct ParsedFragment {
    file: Utf8PathBuf,
    fragment: Node<ast::FragmentDefinition>,
    direct_fragment_spreads: BTreeSet<String>,
}

#[derive(Debug, Default)]
pub(super) struct ParsedInputs {
    operations: BTreeMap<String, ParsedOperation>,
    fragments: BTreeMap<String, ParsedFragment>,
}

impl ParsedInputs {
    pub(super) fn from_file(file: &Utf8Path) -> Result<Self, ParseFailure> {
        let contents = Fs::read_file(file).map_err(|err| ParseFailure {
            file: file.to_path_buf(),
            message: err.to_string(),
        })?;
        let document = ApolloParser::new()
            .parse_ast(contents, file.as_std_path())
            .map_err(|err| ParseFailure {
                file: file.to_path_buf(),
                message: err.to_string(),
            })?;

        let mut parsed = Self::default();
        for definition in document.definitions {
            match definition {
                ast::Definition::OperationDefinition(operation) => {
                    let name = operation
                        .name
                        .as_ref()
                        .map(ToString::to_string)
                        .ok_or_else(|| ParseFailure {
                            file: file.to_path_buf(),
                            message: GenerateError::AnonymousOperation {
                                file: file.to_path_buf(),
                                operation_type: operation.operation_type.to_string(),
                            }
                            .to_string(),
                        })?;
                    if parsed.operations.contains_key(&name) {
                        return Err(ParseFailure {
                            file: file.to_path_buf(),
                            message: GenerateError::DuplicateOperation {
                                name,
                                first_file: file.to_path_buf(),
                                second_file: file.to_path_buf(),
                            }
                            .to_string(),
                        });
                    }
                    parsed.operations.insert(
                        name,
                        ParsedOperation {
                            file: file.to_path_buf(),
                            direct_fragment_spreads: operation.selection_set.collect_spreads(),
                            operation,
                        },
                    );
                }
                ast::Definition::FragmentDefinition(fragment) => {
                    if parsed.fragments.contains_key(fragment.name.as_str()) {
                        return Err(ParseFailure {
                            file: file.to_path_buf(),
                            message: GenerateError::DuplicateFragment {
                                name: fragment.name.to_string(),
                                first_file: file.to_path_buf(),
                                second_file: file.to_path_buf(),
                            }
                            .to_string(),
                        });
                    }
                    parsed.fragments.insert(
                        fragment.name.to_string(),
                        ParsedFragment {
                            file: file.to_path_buf(),
                            direct_fragment_spreads: fragment.selection_set.collect_spreads(),
                            fragment,
                        },
                    );
                }
                _ => {}
            }
        }
        Ok(parsed)
    }

    pub(super) fn from_files(files: Vec<Utf8PathBuf>) -> RoverResult<Self> {
        let (parsed, failures): (Vec<_>, Vec<_>) = files
            .into_iter()
            .map(|file| Self::from_file(&file))
            .partition_result();

        if !failures.is_empty() {
            Err(GenerateError::ParseFailures {
                parse_failures: failures,
            })?;
        }

        parsed
            .into_iter()
            .try_fold(Self::default(), |mut acc, file| {
                acc.merge(file)?;
                Ok(acc)
            })
    }

    pub(super) fn merge(&mut self, other: ParsedInputs) -> RoverResult<()> {
        for (name, operation) in other.operations {
            if let Some(existing) = self.operations.get(&name) {
                Err(GenerateError::DuplicateOperation {
                    name: name.clone(),
                    first_file: existing.file.clone(),
                    second_file: operation.file.clone(),
                })?;
            }
            self.operations.insert(name, operation);
        }

        for (name, fragment) in other.fragments {
            if let Some(existing) = self.fragments.get(&name) {
                Err(GenerateError::DuplicateFragment {
                    name: name.clone(),
                    first_file: existing.file.clone(),
                    second_file: fragment.file.clone(),
                })?;
            }
            self.fragments.insert(name, fragment);
        }

        Ok(())
    }

    pub(super) fn generate_operations(&self) -> RoverResult<Vec<PersistedQueryOperation>> {
        let mut operation_ids = HashMap::new();
        self.operations
            .iter()
            .map(|(name, operation)| {
                let body = operation.body(name, &self.fragments)?;
                let id = sha256_hex(&body);
                if let Some(existing_operation_name) =
                    operation_ids.insert(id.clone(), name.clone())
                {
                    Err(GenerateError::DuplicateOperationId {
                        id: id.clone(),
                        operation_name: name.clone(),
                        existing_operation_name,
                    })?;
                }
                Ok(PersistedQueryOperation {
                    id,
                    name: name.clone(),
                    operation_type: operation.operation.operation_type.name(),
                    body,
                })
            })
            .collect()
    }
}

#[derive(Debug, Serialize)]
pub(super) struct PersistedQueryOperation {
    id: String,
    name: String,
    #[serde(rename = "type")]
    operation_type: &'static str,
    body: String,
}

fn sha256_hex(body: &str) -> String {
    Sha256::digest(body.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    fn parsed_inputs(source: &str) -> ParsedInputs {
        parsed_inputs_from_files(&[("ops.graphql", source)])
    }

    fn parsed_inputs_from_files(files: &[(&str, &str)]) -> ParsedInputs {
        let temp = tempfile::tempdir().unwrap();
        let mut inputs = ParsedInputs::default();
        for (filename, source) in files {
            let file = Utf8PathBuf::from_path_buf(temp.path().join(filename)).unwrap();
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, source).unwrap();
            let parsed_file = ParsedInputs::from_file(&file).unwrap();
            inputs.merge(parsed_file).unwrap();
        }
        inputs
    }

    #[test]
    fn generated_body_matches_default_typescript_manifest_formatting() {
        let inputs = parsed_inputs(indoc::indoc! {r#"
            fragment ProductFields on Product {
              id
              name
              nested { value }
            }

            query GetProduct($id: ID!) {
              product(id: $id) {
                ...ProductFields
              }
            }

            mutation SaveProduct {
              saveProduct(input: { name: "x" }) { id }
            }
        "#});

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations.len()).is_equal_to(2);
        assert_that!(operations[0].name.as_str()).is_equal_to("GetProduct");
        assert_that!(operations[0].operation_type).is_equal_to("query");
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {r#"
            query GetProduct($id: ID!) {
              product(id: $id) {
                ...ProductFields
              }
            }

            fragment ProductFields on Product {
              id
              name
              nested {
                value
              }
            }"#});

        assert_that!(operations[1].name.as_str()).is_equal_to("SaveProduct");
        assert_that!(operations[1].operation_type).is_equal_to("mutation");
        assert_that!(operations[1].body.as_str()).is_equal_to(indoc::indoc! {r#"
            mutation SaveProduct {
              saveProduct(input: {name: "x"}) {
                id
              }
            }"#});
    }

    #[test]
    fn complex_documents_match_default_typescript_manifest_formatting() {
        let inputs = parsed_inputs_from_files(&[
            (
                "complex.graphql",
                indoc::indoc! {r#"
                    query ComplexQuery(
                      $id: ID!
                      $limit: Int = 10
                      $tags: [String!] = ["featured", "sale"]
                      $filter: FilterInput = {status: ACTIVE, range: {min: 1.5, max: 3}}
                      $enabled: Boolean = true
                    ) @trace(enabled: true) {
                      viewer {
                        primary: user(id: $id, filter: $filter, tags: $tags) @include(if: $enabled) {
                          id
                          profile {
                            displayName
                          }
                          ... on Admin {
                            permissions
                          }
                          ...UserFields
                        }
                      }
                    }
                "#},
            ),
            (
                "fragments/user.graphql",
                indoc::indoc! {"
                    fragment UserFields on User @cache(ttl: 60) {
                      name
                      friends(first: $limit) {
                        nodes {
                          id
                        }
                      }
                      ...SharedFields
                    }
                "},
            ),
            (
                "fragments/shared.graphql",
                "fragment SharedFields on User { status }",
            ),
            (
                "subscription.graphql",
                indoc::indoc! {"
                    subscription UserCreatedSubscription($groupId: ID!) {
                      userCreated(groupId: $groupId) {
                        ...UserFields
                      }
                    }
                "},
            ),
        ]);

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations.len()).is_equal_to(2);
        assert_that!(operations[0].name.as_str()).is_equal_to("ComplexQuery");
        assert_that!(operations[0].operation_type).is_equal_to("query");
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {r#"
            query ComplexQuery($id: ID!, $limit: Int = 10, $tags: [String!] = ["featured", "sale"], $filter: FilterInput = {status: ACTIVE, range: {min: 1.5, max: 3}}, $enabled: Boolean = true) @trace(enabled: true) {
              viewer {
                primary: user(id: $id, filter: $filter, tags: $tags) @include(if: $enabled) {
                  id
                  profile {
                    displayName
                  }
                  ... on Admin {
                    permissions
                  }
                  ...UserFields
                }
              }
            }

            fragment SharedFields on User {
              status
            }

            fragment UserFields on User @cache(ttl: 60) {
              name
              friends(first: $limit) {
                nodes {
                  id
                }
              }
              ...SharedFields
            }"#});

        assert_that!(operations[1].name.as_str()).is_equal_to("UserCreatedSubscription");
        assert_that!(operations[1].operation_type).is_equal_to("subscription");
        assert_that!(operations[1].body.as_str()).is_equal_to(indoc::indoc! {"
            subscription UserCreatedSubscription($groupId: ID!) {
              userCreated(groupId: $groupId) {
                ...UserFields
              }
            }

            fragment SharedFields on User {
              status
            }

            fragment UserFields on User @cache(ttl: 60) {
              name
              friends(first: $limit) {
                nodes {
                  id
                }
              }
              ...SharedFields
            }"});
    }

    #[test]
    fn client_directive_selections_match_default_typescript_transform() {
        let inputs = parsed_inputs(indoc::indoc! {"
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

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery {
              currentUser {
                id
              }
            }"});
    }

    #[test]
    fn client_directive_removal_removes_all_client_fields() {
        let inputs = parsed_inputs(indoc::indoc! {"
            query CurrentUserQuery {
              currentUser {
                localOnly @client
              }
            }
        "});

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery {
              currentUser
            }"});
    }

    #[test]
    fn client_directive_removal_prunes_now_unused_variables() {
        let inputs = parsed_inputs(indoc::indoc! {"
            query CurrentUserQuery($localId: ID!, $userId: ID!) {
              isLoggedIn(id: $localId) @client
              currentUser(id: $userId) {
                id
              }
            }
        "});

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery($userId: ID!) {
              currentUser(id: $userId) {
                id
              }
            }"});
    }

    #[test]
    fn variable_pruning_keeps_fragment_and_directive_variables() {
        let inputs = parsed_inputs(indoc::indoc! {"
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

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
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

    #[test]
    fn block_string_literals_match_default_typescript_manifest_formatting() {
        let inputs = parsed_inputs(indoc::indoc! {r#"
            query BlockStringQuery {
              search(text: """hello
            world""") {
                id
              }
            }
        "#});

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {r#"
            query BlockStringQuery {
              search(text: "hello\nworld") {
                id
              }
            }"#});
    }

    #[test]
    fn export_directive_selection_sets_match_default_typescript_transform() {
        let inputs = parsed_inputs(indoc::indoc! {r#"
            query ExportQuery {
              currentUser @export(as: "currentUser") {
                id
                profile {
                  name
                }
              }
              user(id: $currentUser) {
                name
              }
            }
        "#});

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {r#"
            query ExportQuery {
              currentUser @export(as: "currentUser") {
                id
                profile {
                  name
                }
              }
              user(id: $currentUser) {
                name
              }
            }"#});
    }

    #[test]
    fn anonymous_operation_in_file_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let file = Utf8PathBuf::from_path_buf(temp.path().join("ops.graphql")).unwrap();
        std::fs::write(&file, "query { field }").unwrap();
        let result = ParsedInputs::from_file(&file);
        assert_that!(result).is_err();
        assert_that!(result.unwrap_err().to_string()).contains("Please name your query");
    }

    #[test]
    fn duplicate_operation_in_same_file_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let file = Utf8PathBuf::from_path_buf(temp.path().join("ops.graphql")).unwrap();
        std::fs::write(&file, "query GetUser { id }\nquery GetUser { name }").unwrap();
        let result = ParsedInputs::from_file(&file);
        assert_that!(result).is_err();
        assert_that!(result.unwrap_err().to_string()).contains("GetUser");
    }

    #[test]
    fn duplicate_operation_across_files_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let mut combined = ParsedInputs::default();
        for (name, src) in &[
            ("a.graphql", "query GetUser { id }"),
            ("b.graphql", "query GetUser { name }"),
        ] {
            let file = Utf8PathBuf::from_path_buf(temp.path().join(name)).unwrap();
            std::fs::write(&file, src).unwrap();
            let parsed = ParsedInputs::from_file(&file).unwrap();
            if combined.merge(parsed).is_err() {
                return; // expected error path reached
            }
        }
        panic!("expected duplicate operation error");
    }

    #[test]
    fn duplicate_fragment_across_files_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let mut combined = ParsedInputs::default();
        for (name, src) in &[
            ("a.graphql", "fragment F on T { id }"),
            ("b.graphql", "fragment F on T { name }"),
        ] {
            let file = Utf8PathBuf::from_path_buf(temp.path().join(name)).unwrap();
            std::fs::write(&file, src).unwrap();
            let parsed = ParsedInputs::from_file(&file).unwrap();
            if combined.merge(parsed).is_err() {
                return; // expected error path reached
            }
        }
        panic!("expected duplicate fragment error");
    }

    #[test]
    fn missing_fragment_reference_returns_error() {
        let inputs = parsed_inputs("query GetUser { ...UserFields }");
        let result = inputs.generate_operations();
        assert_that!(result).is_err();
        assert_that!(result.unwrap_err().to_string()).contains("UserFields");
    }

    #[test]
    fn reachable_fragments_are_sorted_by_name_and_transitive() {
        let inputs = parsed_inputs(indoc::indoc! {"
            fragment Zed on Product { z }
            fragment Alpha on Product { a ...Zed }
            query GetProduct { product { ...Alpha } }
        "});

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
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
}
