use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use rover::PKG_VERSION;

#[test]
fn it_prints_info() {
    let mut cmd = cargo_bin_cmd!("rover");
    let result = cmd.arg("info").assert().success();

    // the version should always be available in the `info` output
    result.stderr(predicate::str::contains(PKG_VERSION));
}
