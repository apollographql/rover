use std::{
    io::Write,
    process::{Command as StdCommand, Stdio},
};

use assert_cmd::Command;
use predicates::prelude::*;
use which::which;

#[test]
fn it_generates_bash_completion() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.args(["completion", "bash"]).assert().success();

    // Bash completion scripts should contain function definitions
    result.stdout(predicate::str::contains("_rover"));

    // Validate bash syntax by piping output to bash -n
    // Skip syntax validation if bash is not available (e.g., in some CI environments)
    if which("bash").is_ok() {
        let mut rover_cmd = Command::cargo_bin("rover").unwrap();
        let rover_output = rover_cmd
            .args(["completion", "bash"])
            .output()
            .expect("Failed to run rover completion bash");
        assert!(
            rover_output.status.success(),
            "rover completion bash should succeed"
        );

        let mut bash_cmd = StdCommand::new("bash")
            .arg("-n")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn bash");

        bash_cmd
            .stdin
            .as_mut()
            .expect("Failed to get bash stdin")
            .write_all(&rover_output.stdout)
            .expect("Failed to write to bash stdin");

        let bash_result = bash_cmd
            .wait_with_output()
            .expect("Failed to wait for bash");
        assert!(
            bash_result.status.success(),
            "bash syntax check failed. stderr: {}",
            String::from_utf8_lossy(&bash_result.stderr)
        );
    } else {
        eprintln!("Skipping bash syntax validation: bash not installed");
    }
}

#[test]
fn it_generates_zsh_completion() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.args(["completion", "zsh"]).assert().success();

    // Zsh completion scripts should contain completion function definitions
    result.stdout(predicate::str::contains("_rover"));

    // Validate zsh syntax by piping output to zsh -n
    // Skip syntax validation if zsh is not available (e.g., in some CI environments)
    if which("zsh").is_ok() {
        let mut rover_cmd = Command::cargo_bin("rover").unwrap();
        let rover_output = rover_cmd
            .args(["completion", "zsh"])
            .output()
            .expect("Failed to run rover completion zsh");
        assert!(
            rover_output.status.success(),
            "rover completion zsh should succeed"
        );

        let mut zsh_cmd = StdCommand::new("zsh")
            .arg("-n")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn zsh");

        zsh_cmd
            .stdin
            .as_mut()
            .expect("Failed to get zsh stdin")
            .write_all(&rover_output.stdout)
            .expect("Failed to write to zsh stdin");

        let zsh_result = zsh_cmd.wait_with_output().expect("Failed to wait for zsh");
        assert!(
            zsh_result.status.success(),
            "zsh syntax check failed. stderr: {}",
            String::from_utf8_lossy(&zsh_result.stderr)
        );
    } else {
        eprintln!("Skipping zsh syntax validation: zsh not installed");
    }
}
