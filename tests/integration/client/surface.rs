use assert_cmd::Command;

#[test]
fn client_check_help_works() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("client")
        .arg("check")
        .arg("--help")
        .assert()
        .success();
}
