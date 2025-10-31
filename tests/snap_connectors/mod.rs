#[test]
fn cli_tests() {
    trycmd::TestCases::new()
        .case("tests/snap_connectors/cmd/*.md")
        .insert_var("[REPLACEMENT]", "runtime-value").unwrap();
}
