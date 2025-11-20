#[test]
fn cli_tests() {
    trycmd::TestCases::new()
        .case("e2e/connectors/*.md")
        .case("e2e/introspection/*.md");
}