#[test]
#[ignore]
fn connectors_cli_tests() {
    trycmd::TestCases::new().case("e2e/connectors/*.md");
}

#[test]
#[ignore]
fn introspection_cli_tests() {
    trycmd::TestCases::new().case("e2e/introspection/*.md");
}
