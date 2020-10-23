use assert_cmd::Command;

#[test]
fn its_has_a_schema_fetch_command() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("schema")
        .arg("fetch")
        .arg("--help")
        .assert()
        .success();
}
