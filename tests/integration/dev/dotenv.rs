use assert_cmd::{Command, cargo::cargo_bin_cmd};
use predicates::prelude::{PredicateBooleanExt, predicate};

/// A `rover` run in a project directory whose `.env` holds `dotenv`, with nothing that could
/// stand in for the settings under test left in the real environment.
fn rover_with_dotenv(
    dotenv: &str,
    project: &tempfile::TempDir,
    config_home: &tempfile::TempDir,
) -> Command {
    std::fs::write(project.path().join(".env"), dotenv).unwrap();
    let mut rover = cargo_bin_cmd!("rover");
    rover
        .current_dir(project.path())
        .env_remove("APOLLO_ELV2_LICENSE")
        .env_remove("APOLLO_GRAPH_REF")
        .env_remove("APOLLO_KEY")
        .env_remove("APOLLO_LOG_LEVEL")
        .env("APOLLO_CONFIG_HOME", config_home.path())
        .env("NO_COLOR", "1")
        .arg("--telemetry-disabled");
    rover
}

/// A graph ref in `.env` is used by `rover dev` as one in the environment is, so an invalid one
/// is reported.
#[test]
fn dev_uses_a_graph_ref_from_dotenv() {
    let project = tempfile::tempdir().unwrap();
    let config_home = tempfile::tempdir().unwrap();

    rover_with_dotenv("APOLLO_GRAPH_REF=bad!!\n", &project, &config_home)
        .arg("dev")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error[E054]"))
        .stderr(predicate::str::contains("`APOLLO_GRAPH_REF`"));
}

/// A flag that clap reads from its environment variable is read from `.env` too, so an invalid
/// value is rejected as it would be from the environment.
#[test]
fn dev_uses_a_flag_value_from_dotenv() {
    let project = tempfile::tempdir().unwrap();
    let config_home = tempfile::tempdir().unwrap();

    rover_with_dotenv("APOLLO_ELV2_LICENSE=bogus\n", &project, &config_home)
        .arg("dev")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'bogus'"));
}

/// A variable that is already set in the environment wins over the same one in `.env`.
#[test]
fn the_environment_wins_over_dotenv() {
    let project = tempfile::tempdir().unwrap();
    let config_home = tempfile::tempdir().unwrap();

    rover_with_dotenv("APOLLO_ELV2_LICENSE=bogus\n", &project, &config_home)
        .env("APOLLO_ELV2_LICENSE", "accept")
        .arg("dev")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value").not());
}

/// Only `rover dev` reads `.env`, so another command is unaffected by one.
#[test]
fn another_command_ignores_dotenv() {
    let project = tempfile::tempdir().unwrap();
    let config_home = tempfile::tempdir().unwrap();

    rover_with_dotenv("APOLLO_LOG_LEVEL=bogus\n", &project, &config_home)
        .args(["config", "list"])
        .assert()
        .success();
}
