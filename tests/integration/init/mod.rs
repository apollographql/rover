use assert_cmd::Command;
use speculoos::prelude::*;

/// With no credential, `rover init` asks for a pasted key. With no terminal to ask on that can't
/// work, and used to fail with "Unexpected system error: IO error: not a terminal ... This isn't
/// your fault!" instead of saying what to do.
#[test]
fn init_without_a_credential_or_a_terminal_says_how_to_log_in() {
    let config_home = tempfile::tempdir().unwrap();
    let working_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .current_dir(working_dir.path())
        .arg("--config-home")
        .arg(config_home.path())
        .args(["init", "--skip-update-check", "--telemetry-disabled"])
        .env_remove("APOLLO_KEY")
        .env_remove("APOLLO_CLIENT_ID")
        .env_remove("APOLLO_CLIENT_SECRET")
        .env("NO_COLOR", "1")
        .write_stdin("")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_that!(output.status.success()).is_false();
    assert_that!(stderr.contains("no terminal"))
        .named(&stderr)
        .is_true();
    assert_that!(stderr.contains("rover auth login"))
        .named(&stderr)
        .is_true();
    assert_that!(stderr.contains("Unexpected system error"))
        .named(&stderr)
        .is_false();
}
