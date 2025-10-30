#[test]
fn cli_tests() {
    trycmd::TestCases::new()
        .case("tests/snap_connectors/cmd/*.toml")
        .case("tests/snap_connectors/cmd/*.md");
}
