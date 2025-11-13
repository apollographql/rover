use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn it_generates_bash_completion() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.args(["completion", "bash"]).assert().success();

    // Bash completion scripts should contain function definitions
    result.stdout(predicate::str::contains("_rover"));
}

#[test]
fn it_generates_zsh_completion() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.args(["completion", "zsh"]).assert().success();

    // Zsh completion scripts should contain completion function definitions
    result.stdout(predicate::str::contains("_rover"));
}
