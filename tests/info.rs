use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn it_prints_info() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.arg("info").assert().success();

    // the version should always be available in the `info` output
    let version = env!("CARGO_PKG_VERSION");
    result.stderr(predicate::str::contains(version));
}
