use assert_cmd::cargo::cargo_bin_cmd;
use rstest::rstest;

#[rstest]
#[ignore]
fn e2e_test_rover_init_help() {
    let mut cmd = cargo_bin_cmd!("rover");
    cmd.arg("init").arg("--help").assert().success();
}
