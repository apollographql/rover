#[test]
#[ignore]
fn cli_tests() {
    trycmd::TestCases::new().case("e2e/**/*.md");
}
