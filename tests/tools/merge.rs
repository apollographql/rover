use assert_cmd::Command;

#[test]
fn it_has_a_tools_merge_command() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("tools")
        .arg("merge")
        .arg("--help")
        .assert()
        .success();
}