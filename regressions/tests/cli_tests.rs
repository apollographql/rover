#[test]
fn introspection_cli_tests() {
    trycmd::TestCases::new().case("e2e/introspection/*.md");
}

#[test]
fn connectors_cli_tests() {
    trycmd::TestCases::new().case("e2e/connectors/*.md");
}
