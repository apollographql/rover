mod parsed_fragment;
mod parsed_inputs;
mod parsed_operation;
mod persisted_query_operation;

pub(super) use parsed_inputs::ParsedInputs;
pub(super) use persisted_query_operation::PersistedQueryOperation;

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
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
    fn duplicate_operation_id_returns_error() {
        let inputs = parsed_inputs_from_files(&[
            ("a.graphql", "query GetFoo { id }"),
            ("b.graphql", "query GetBar { id }"),
        ]);
        let result = inputs.generate_operations_with_id(|_| "fixed-id".to_string());
        assert_that!(result).is_err();
        let msg = result.unwrap_err().to_string();
        assert_that!(msg).contains("fixed-id");
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
}
