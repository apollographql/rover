use assert_cmd::Command;
use predicates::prelude::*;
use rover::{PKG_NAME, PKG_VERSION};

#[test]
fn it_prints_info() {
    let mut cmd = Command::cargo_bin(PKG_NAME).unwrap();
    let result = cmd.arg("info").assert().success();

    // the version should always be available in the `info` output
    result.stderr(predicate::str::contains(PKG_VERSION));
}
