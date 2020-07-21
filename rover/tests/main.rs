use assert_cmd::Command;

#[test]
fn its_executable() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.assert().success();
}
