#[test]
fn cli_tests() {
    trycmd::TestCases::new()
        .case("tests/e2e_connectors/cmd/*.toml")
        .case("tests/e2e_connectors/cmd/*.md");
}
