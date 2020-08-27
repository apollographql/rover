mod config;
mod schema;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn its_executable() {
    let mut cmd = Command::cargo_bin("rover").unwrap();

    // running the CLI with no command returns to std err
    let result = cmd.assert();
    result.stderr(predicate::str::contains("USAGE"));
}
