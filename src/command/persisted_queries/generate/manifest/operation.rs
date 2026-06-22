use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use apollo_compiler::{Node, ast, parser::Parser as ApolloParser};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use rover_std::Fs;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    super::printer::{PrintableDefinition, operation_type_str, print_document},
    ast_ext::{OperationDefinitionExt, SelectionSetExt},
    error::{GenerateError, GenerateFailure},
};
use crate::RoverResult;

#[derive(Debug, Clone)]
struct ParsedOperation {
    file: Utf8PathBuf,
    source: Arc<str>,
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
        operation_node.make_mut().add_typenames();

        let fragment_definitions: Vec<(Node<ast::FragmentDefinition>, Arc<str>)> = reachable
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
                fragment_definition.selection_set.add_typenames();
                (fragment_node, Arc::clone(&fragment.source))
            })
            .collect();

        operation_node
            .make_mut()
            .prune_unused_variables(&fragment_definitions);

        let definitions = std::iter::once(PrintableDefinition::Operation {
            operation: operation_node,
            source: Arc::clone(&self.source),
        })
        .chain(
            fragment_definitions
                .into_iter()
                .map(|(fragment, source)| PrintableDefinition::Fragment { fragment, source }),
        )
        .collect::<Vec<_>>();

        Ok(print_document(&definitions))
    }
}

#[derive(Debug, Clone)]
struct ParsedFragment {
    file: Utf8PathBuf,
    source: Arc<str>,
    fragment: Node<ast::FragmentDefinition>,
    direct_fragment_spreads: BTreeSet<String>,
}

#[derive(Debug, Default)]
pub(super) struct ParsedInputs {
    operations: BTreeMap<String, ParsedOperation>,
    fragments: BTreeMap<String, ParsedFragment>,
}

impl ParsedInputs {
    pub(super) fn from_file(file: &Utf8Path) -> Result<Self, GenerateFailure> {
        let contents = Fs::read_file(file).map_err(|err| GenerateFailure {
            file: file.to_path_buf(),
            message: err.to_string(),
        })?;
        let source: Arc<str> = Arc::from(contents.as_str());
        let document = ApolloParser::new()
            .parse_ast(contents, file.as_std_path())
            .map_err(|err| GenerateFailure {
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
                        .ok_or_else(|| GenerateFailure {
                            file: file.to_path_buf(),
                            message: GenerateError::AnonymousOperation {
                                file: file.to_path_buf(),
                                operation_type: operation.operation_type.to_string(),
                            }
                            .to_string(),
                        })?;
                    if parsed.operations.contains_key(&name) {
                        return Err(GenerateFailure {
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
                            source: Arc::clone(&source),
                            direct_fragment_spreads: operation.selection_set.collect_spreads(),
                            operation,
                        },
                    );
                }
                ast::Definition::FragmentDefinition(fragment) => {
                    if parsed.fragments.contains_key(fragment.name.as_str()) {
                        return Err(GenerateFailure {
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
                            source: Arc::clone(&source),
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
                    operation_type: operation_type_str(operation.operation.operation_type),
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
                __typename
              }
            }

            fragment ProductFields on Product {
              id
              name
              nested {
                value
                __typename
              }
              __typename
            }"#});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("deca7ebeb3e6d8e46f056fdc032ed462dc6a9763d9225eb04ab9e9943b6c248a");

        assert_that!(operations[1].name.as_str()).is_equal_to("SaveProduct");
        assert_that!(operations[1].operation_type).is_equal_to("mutation");
        assert_that!(operations[1].body.as_str()).is_equal_to(indoc::indoc! {r#"
            mutation SaveProduct {
              saveProduct(input: {name: "x"}) {
                id
                __typename
              }
            }"#});
        assert_that!(operations[1].id.as_str())
            .is_equal_to("e2cae5428130630ffe997257613154698cd85f7ef97c4ffe653ca80183b8e10f");
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
                    __typename
                  }
                  ... on Admin {
                    permissions
                    __typename
                  }
                  ...UserFields
                  __typename
                }
                __typename
              }
            }

            fragment SharedFields on User {
              status
              __typename
            }

            fragment UserFields on User @cache(ttl: 60) {
              name
              friends(first: $limit) {
                nodes {
                  id
                  __typename
                }
                __typename
              }
              ...SharedFields
              __typename
            }"#});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("4501a1585e6aaf2adea38c6ffc4114135b71871e69bd43fff71de6a4ce8b57c2");

        assert_that!(operations[1].name.as_str()).is_equal_to("UserCreatedSubscription");
        assert_that!(operations[1].operation_type).is_equal_to("subscription");
        assert_that!(operations[1].body.as_str()).is_equal_to(indoc::indoc! {"
            subscription UserCreatedSubscription($groupId: ID!) {
              userCreated(groupId: $groupId) {
                ...UserFields
                __typename
              }
            }

            fragment SharedFields on User {
              status
              __typename
            }

            fragment UserFields on User @cache(ttl: 60) {
              name
              friends(first: $limit) {
                nodes {
                  id
                  __typename
                }
                __typename
              }
              ...SharedFields
              __typename
            }"});
        assert_that!(operations[1].id.as_str())
            .is_equal_to("e936af1be273b8d80d7c06927423827cbe464c3efd6b67ab02e948d20c3c9b59");
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
                __typename
              }
            }"});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("2bc729f3095726f8bc03301874e1e185d22aa06aad024b49c868a641c24c1902");
    }

    #[test]
    fn client_directive_removal_preserves_nested_typename() {
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
              currentUser {
                __typename
              }
            }"});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("92e0c664584eac8c318fd0193771ceab698eb53b55f9cbe5e8f82a7935086c7e");
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
                __typename
              }
            }"});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("a009379fd75dbf344e170f04bca196eb6d3ba5aff06eef54b0a6129a51bd11c9");
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
                __typename
              }
            }

            fragment UserFields on User {
              friends(first: $userId) {
                id
                __typename
              }
              __typename
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
              search(text: """
              hello
              world
              """) {
                id
                __typename
              }
            }"#});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("5d355c2a5cf2e2358f47521d303e0aaa4c5d5853e1b24454ed4170291b7c0a18");
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
                  __typename
                }
              }
              user(id: $currentUser) {
                name
                __typename
              }
            }"#});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("235f5fc1cc144ac4e7484faf86266e6e393679e3c268b739abd3422a53adcd07");
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
                __typename
              }
            }

            fragment Alpha on Product {
              a
              ...Zed
              __typename
            }

            fragment Zed on Product {
              z
              __typename
            }"});
    }
}
