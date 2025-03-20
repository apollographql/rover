use assert_cmd::Command;
use rstest::rstest;

#[rstest]
#[ignore]
fn e2e_test_rover_init_help() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("init")
        .arg("--help")
        .assert()
        .success();
}
