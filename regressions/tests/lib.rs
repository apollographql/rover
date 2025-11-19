#[test]
fn cli_tests() {
    trycmd::TestCases::new().case("e2e/**/*.md");
}
