mod comments;
mod parsed_fragment;
mod parsed_inputs;
mod parsed_operation;
mod persisted_query_operation;

pub(super) use parsed_inputs::ParsedInputs;
pub(super) use persisted_query_operation::PersistedQueryOperation;

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    fn assert_operation_bodies_and_ids(
        operations: &[PersistedQueryOperation],
        expected: &[(&str, &str, &str)],
    ) {
        let actual: Vec<_> = operations
            .iter()
            .map(|operation| {
                (
                    operation.name.as_str(),
                    operation.body.as_str(),
                    operation.id.as_str(),
                )
            })
            .collect();
        assert_that!(actual).is_equal_to(expected.to_vec());
    }

    fn parsed_inputs(source: &str) -> ParsedInputs {
        parsed_inputs_from_files(&[("ops.graphql", source)])
    }

    fn parsed_inputs_from_files(files: &[(&str, &str)]) -> ParsedInputs {
        parsed_inputs_from_files_with_comments(files, false)
    }

    fn parsed_inputs_from_files_with_comments(
        files: &[(&str, &str)],
        preserve_comments: bool,
    ) -> ParsedInputs {
        let temp = tempfile::tempdir().unwrap();
        let mut inputs = ParsedInputs::default();
        for (filename, source) in files {
            let file = Utf8PathBuf::from_path_buf(temp.path().join(filename)).unwrap();
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, source).unwrap();
            let parsed_file = ParsedInputs::from_file(&file, preserve_comments).unwrap();
            inputs.merge(parsed_file).unwrap();
        }
        inputs
    }

    #[test]
    fn preserved_comments_appear_above_the_operation_body() {
        let inputs = parsed_inputs_from_files_with_comments(
            &[(
                "ops.graphql",
                indoc::indoc! {"
                    # Home feed entry point.
                    # @meta fci_tier: TIER_1
                    query GetHomeFeed($userId: ID!) {
                      homeFeed(userId: $userId) { id }
                    }
                "},
            )],
            true,
        );

        let operations = inputs.generate_operations().unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
            # Home feed entry point.
            # @meta fci_tier: TIER_1
            query GetHomeFeed($userId: ID!) {
              homeFeed(userId: $userId) {
                id
              }
            }"});
    }

    #[test]
    fn comments_are_dropped_unless_preservation_is_enabled() {
        let source = "# @meta fci_tier: TIER_1\nquery GetFoo { id }";

        let dropped = parsed_inputs_from_files_with_comments(&[("ops.graphql", source)], false)
            .generate_operations()
            .unwrap();
        let preserved = parsed_inputs_from_files_with_comments(&[("ops.graphql", source)], true)
            .generate_operations()
            .unwrap();

        assert_that!(dropped[0].body.as_str()).is_equal_to(indoc::indoc! {"
            query GetFoo {
              id
            }"});
        assert_that!(preserved[0].body.as_str()).is_equal_to(indoc::indoc! {"
            # @meta fci_tier: TIER_1
            query GetFoo {
              id
            }"});
    }

    #[test]
    fn preserving_comments_changes_only_annotated_operation_ids() {
        let source = indoc::indoc! {"
            # @meta fci_tier: TIER_1
            query Annotated { id }

            query Bare { id }
        "};

        let dropped = parsed_inputs_from_files_with_comments(&[("ops.graphql", source)], false)
            .generate_operations()
            .unwrap();
        let preserved = parsed_inputs_from_files_with_comments(&[("ops.graphql", source)], true)
            .generate_operations()
            .unwrap();

        assert_operation_bodies_and_ids(
            &dropped,
            &[
                (
                    "Annotated",
                    "query Annotated {\n  id\n}",
                    "374b850ff56be132ad7387455a0ad55edc339ce9d282dfc824c1a9cf11378126",
                ),
                (
                    "Bare",
                    "query Bare {\n  id\n}",
                    "134756411986eda7c8dfe928886208381ba772e16d277de127c136f133d1d5c5",
                ),
            ],
        );
        assert_operation_bodies_and_ids(
            &preserved,
            &[
                (
                    "Annotated",
                    "# @meta fci_tier: TIER_1\nquery Annotated {\n  id\n}",
                    "27670d8decfe42cf4e3923d86f50cc4680e3ce4967a83e5cf515f66863f5bd56",
                ),
                (
                    "Bare",
                    "query Bare {\n  id\n}",
                    "134756411986eda7c8dfe928886208381ba772e16d277de127c136f133d1d5c5",
                ),
            ],
        );
    }

    #[rstest]
    #[case::same_line("# A metadata\nquery A { a } query B { b }")]
    #[case::trailing_comment("# A metadata\nquery A { a } # trailing\nquery B { b }")]
    fn operation_comments_stay_with_their_owner(
        #[case] source: &str,
        #[values(false, true)] preserve_comments: bool,
    ) {
        let operations =
            parsed_inputs_from_files_with_comments(&[("ops.graphql", source)], preserve_comments)
                .generate_operations()
                .unwrap();
        let (body_a, id_a) = if preserve_comments {
            (
                "# A metadata\nquery A {\n  a\n}",
                "38743c67180c36fdc6a22ce573b3ade9b8859f6942abf324cf8f57ae6927e5d3",
            )
        } else {
            (
                "query A {\n  a\n}",
                "a2e93ae31651942b495db72b919ced0636a2450954bffd338df4c1578380b9fc",
            )
        };
        assert_operation_bodies_and_ids(
            &operations,
            &[
                ("A", body_a, id_a),
                (
                    "B",
                    "query B {\n  b\n}",
                    "967c6a00169d201fe04191715aa8f83cd6170b22f5c748ec0d8ccd39bfdb3b35",
                ),
            ],
        );
    }

    #[rstest]
    fn same_line_fragment_comments_do_not_become_operation_comments(
        #[values(false, true)] preserve_comments: bool,
    ) {
        let operations = parsed_inputs_from_files_with_comments(
            &[(
                "ops.graphql",
                "# Fragment metadata\nfragment F on X { x } query Q { ...F }",
            )],
            preserve_comments,
        )
        .generate_operations()
        .unwrap();
        assert_operation_bodies_and_ids(
            &operations,
            &[(
                "Q",
                "query Q {\n  ...F\n}\n\nfragment F on X {\n  x\n}",
                "b0a98ab77022a1d79aa720917423dbb419f71a245ef6a6136294939b3f761cc7",
            )],
        );
    }

    #[rstest]
    fn block_string_text_does_not_become_another_operations_comment(
        #[values(false, true)] preserve_comments: bool,
    ) {
        let operations = parsed_inputs_from_files_with_comments(
            &[(
                "ops.graphql",
                "query A { f(arg: \"\"\"\n# string content\n\"\"\") } query B { b }",
            )],
            preserve_comments,
        )
        .generate_operations()
        .unwrap();
        assert_operation_bodies_and_ids(
            &operations,
            &[
                (
                    "A",
                    "query A {\n  f(arg: \"# string content\")\n}",
                    "701a34768698fd540db186c1790bda8dc494d9bc303d0a4e1f416a42e09725f0",
                ),
                (
                    "B",
                    "query B {\n  b\n}",
                    "967c6a00169d201fe04191715aa8f83cd6170b22f5c748ec0d8ccd39bfdb3b35",
                ),
            ],
        );
    }

    #[rstest]
    #[case::operation("query A { f(arg: \"\"\"\n# string content\"\"\") }", true)]
    #[case::input_default("input I { value: String = \"\"\"\n# string content\"\"\" }", false)]
    #[case::type_description("\"\"\"\n# description\"\"\" type T { x: String }", false)]
    fn preceding_block_strings_do_not_become_operation_comments(
        #[case] preceding_definition: &str,
        #[case] has_operation: bool,
        #[values(false, true)] annotated: bool,
        #[values(false, true)] preserve_comments: bool,
    ) {
        let comment = if annotated { "# B metadata\n" } else { "" };
        let source = format!("{preceding_definition}\n{comment}query B {{ b }}");
        let operations =
            parsed_inputs_from_files_with_comments(&[("ops.graphql", &source)], preserve_comments)
                .generate_operations()
                .unwrap();
        let mut expected = Vec::new();
        if has_operation {
            expected.push((
                "A",
                "query A {\n  f(arg: \"# string content\")\n}",
                "701a34768698fd540db186c1790bda8dc494d9bc303d0a4e1f416a42e09725f0",
            ));
        }
        let (body_b, id_b) = if annotated && preserve_comments {
            (
                "# B metadata\nquery B {\n  b\n}",
                "f46fa13b6d262385dc829fdc847d25fee4b6a6e27e630b4032e4dddaece63a20",
            )
        } else {
            (
                "query B {\n  b\n}",
                "967c6a00169d201fe04191715aa8f83cd6170b22f5c748ec0d8ccd39bfdb3b35",
            )
        };
        expected.push(("B", body_b, id_b));
        assert_operation_bodies_and_ids(&operations, &expected);
    }

    #[rstest]
    #[case::lf("# metadata\nquery Q { x }")]
    #[case::cr("# metadata\rquery Q { x }")]
    #[case::crlf("# metadata\r\nquery Q { x }")]
    #[case::bom_before_comment("\u{feff}# metadata\nquery Q { x }")]
    #[case::bom_before_operation("# metadata\n \t\u{feff}query Q { x }")]
    #[case::comma_before_operation("# metadata\n, \u{feff}query Q { x }")]
    fn comment_body_and_id_are_stable_across_ignored_source_characters(
        #[case] source: &str,
        #[values(false, true)] preserve_comments: bool,
    ) {
        let operations =
            parsed_inputs_from_files_with_comments(&[("ops.graphql", source)], preserve_comments)
                .generate_operations()
                .unwrap();
        let (body, id) = if preserve_comments {
            (
                "# metadata\nquery Q {\n  x\n}",
                "779e5e3b0fdf321abd8932bcae41c6fea1b29feb4c63bfd80d29e746ed2b8f06",
            )
        } else {
            (
                "query Q {\n  x\n}",
                "8e0b02444ba4ea82e7e226c93c607666ac71e2a020ed2ecfe09c747b03bfb40e",
            )
        };
        assert_operation_bodies_and_ids(&operations, &[("Q", body, id)]);
    }

    #[rstest]
    fn shared_fragments_do_not_transfer_comments_across_files(
        #[values(false, true)] preserve_comments: bool,
    ) {
        let operations = parsed_inputs_from_files_with_comments(
            &[
                ("a.graphql", "\u{feff}# A metadata\rquery A { ...F }"),
                ("b.graphql", "query B { ...F }"),
                (
                    "fragment.graphql",
                    "# Fragment metadata\nfragment F on X { x }",
                ),
            ],
            preserve_comments,
        )
        .generate_operations()
        .unwrap();
        let (body_a, id_a) = if preserve_comments {
            (
                "# A metadata\nquery A {\n  ...F\n}\n\nfragment F on X {\n  x\n}",
                "3c9344a1b2bce628df4cb5f21eba05aa09729859aea12ecf1ef3f5e8575178b4",
            )
        } else {
            (
                "query A {\n  ...F\n}\n\nfragment F on X {\n  x\n}",
                "f289e745b67abd7a2eabda194dcc8ce870449a74a7ba8d6bd0f753e802ecfb90",
            )
        };
        assert_operation_bodies_and_ids(
            &operations,
            &[
                ("A", body_a, id_a),
                (
                    "B",
                    "query B {\n  ...F\n}\n\nfragment F on X {\n  x\n}",
                    "af9c1b115c7474911deaeadf903552afe7f47dd5ce72556726ae91801463a3df",
                ),
            ],
        );
    }

    #[test]
    fn preserved_comment_bodies_are_still_valid_graphql() {
        let inputs = parsed_inputs_from_files_with_comments(
            &[(
                "ops.graphql",
                indoc::indoc! {"
                    # @meta fci_tier: TIER_1
                    # @meta owner: consumer-payments
                    query GetHomeFeed($userId: ID!) {
                      homeFeed(userId: $userId) { ...FeedFields }
                    }

                    fragment FeedFields on Feed { id title }
                "},
            )],
            true,
        );

        let operations = inputs.generate_operations().unwrap();
        let body = operations[0].body.as_str();

        assert_that!(body).contains("# @meta fci_tier: TIER_1");
        // A body carrying comments must still parse, since the router executes
        // the stored body verbatim.
        let parsed = apollo_compiler::parser::Parser::new().parse_ast(body, "body.graphql");
        assert_that!(parsed.is_ok()).is_true();
    }

    #[test]
    fn comments_above_fragments_are_not_preserved() {
        let inputs = parsed_inputs_from_files_with_comments(
            &[(
                "ops.graphql",
                indoc::indoc! {"
                    # @meta fci_tier: TIER_1
                    query GetProduct { product { ...ProductFields } }

                    # @meta this comment is on a fragment
                    fragment ProductFields on Product { id }
                "},
            )],
            true,
        );

        let operations = inputs.generate_operations().unwrap();
        let body = operations[0].body.as_str();

        assert_that!(body).contains("# @meta fci_tier: TIER_1");
        assert_that!(body).does_not_contain("this comment is on a fragment");
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
