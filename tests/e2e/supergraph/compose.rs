use std::{
    env,
    process::{Command, Stdio},
};

use assert_cmd::cargo;
use regex::{Regex, RegexSet};
use rstest::*;
use serde_json::Value;
use tempfile::TempDir;
use tracing::error;
use tracing_test::traced_test;

use crate::e2e::{RetailSupergraph, retail_supergraph};

#[rstest]
#[ignore]
#[traced_test]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_run_rover_supergraph_compose(retail_supergraph: &RetailSupergraph) {
    // GIVEN
    //   - a supergraph config yaml (fixture)
    //   - retail supergraphs representing any set of subgraphs to be composed into a supergraph
    //   (fixture)
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    let mut args: Vec<String> = vec![
        "supergraph",
        "compose",
        "--config",
        "supergraph-config-dev.yaml",
        "--output",
        "composition-result",
        "--elv2-license",
        "accept",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        args.push("--federation-version".to_string());
        args.push(format!("={version}"));
    };
    cmd.args(args);
    cmd.current_dir(&retail_supergraph.working_dir);
    // The composition plugin is downloaded as the command runs.
    cmd.env("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "true");
    let match_set: Vec<String> = retail_supergraph
        .get_subgraph_names()
        .into_iter()
        .map(|n| format!(r#"@join__graph\(name: "{n}"#))
        .collect();

    let re_set = RegexSet::new(&match_set).unwrap();

    // WHEN
    //   - `rover supergraph compose` is invoked with the supergraph yaml and a flag for writing
    //   composition to disk
    // THEN
    //   - a success code is returned
    match cmd.output() {
        Ok(output) => {
            if !output.status.success() {
                error!("{}", std::str::from_utf8(&output.stderr).unwrap());
                panic!("Supergraph compose command did not execute successfully!");
            }
        }
        Err(err) => {
            panic!("Could not execute `supergraph compose` command\n{err}");
        }
    }

    // AND
    //   - the composition result is saved in the tmp dir
    //   - the composition result joins all the graphs named in the supergraph config
    let composition_result_path = retail_supergraph
        .working_dir
        .path()
        .join("composition-result");
    let composition_result = std::fs::read_to_string(composition_result_path)
        .expect("Could not read composition result file");
    let matched_len: usize = re_set.matches(&composition_result).into_iter().count();
    assert_eq!(matched_len, retail_supergraph.get_subgraph_names().len());
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn it_fails_without_a_config() {
    // GIVEN
    //   - an invocation of `rover supergraph compose` without any config file
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args(["supergraph", "compose"]);
    cmd.stdin(Stdio::null());

    // WHEN
    //   - it's invoked
    let res = cmd.spawn().expect("Could not run rover supergraph command");
    let output = res.wait_with_output();

    // THEN
    //   - a failure  code is returned
    assert!(output.is_ok_and(|code| { code.status.code() == Some(2) }));
}

/// `rover supergraph compose` of the retail supergraph with a fresh
/// `APOLLO_HOME`, so no composition plugin is installed, in JSON, with `env`.
fn compose_with_a_fresh_rover_home(
    retail_supergraph: &RetailSupergraph,
    env: &[(&str, &str)],
) -> (std::process::Output, TempDir) {
    let rover_home = TempDir::new().expect("Could not create temporary directory");
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "supergraph",
        "compose",
        "--config",
        "supergraph-config-dev.yaml",
        "--elv2-license",
        "accept",
        "--skip-update-check",
        "--format",
        "json",
    ]);
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.args(["--federation-version", &format!("={version}")]);
    }
    cmd.current_dir(&retail_supergraph.working_dir)
        .env("APOLLO_HOME", rover_home.path())
        .env_remove("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD")
        .env_remove("APOLLO_ROVER_SKIP_UPDATE")
        .envs(env.iter().copied());
    let output = cmd
        .output()
        .expect("Could not execute `supergraph compose` command");
    (output, rover_home)
}

/// With nothing setting `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD`, compose
/// downloads the composition plugin it needs from the real plugin registry,
/// succeeds, and warns once that a future version of Rover won't.
#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_compose_downloads_with_a_warning_when_nothing_sets_the_setting(
    retail_supergraph: &RetailSupergraph,
) {
    let (output, _rover_home) = compose_with_a_fresh_rover_home(retail_supergraph, &[]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    let warning = Regex::new(concat!(
        r"(?m)^Warning: Rover downloaded the `supergraph` plugin v\S+ because ",
        r"`APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` isn't set\. A future version of Rover will ",
        r"not install plugins automatically\. ",
    ))
    .unwrap();
    assert!(output.status.success(), "compose failed:\n{stderr}");
    assert_eq!(warning.find_iter(&stderr).count(), 1, "{stderr}");
}

/// With `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD=false`, compose doesn't
/// download the composition plugin: it fails with E058 and installs nothing.
#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_compose_with_downloads_turned_off_fails_with_e058(
    retail_supergraph: &RetailSupergraph,
) {
    let (output, rover_home) = compose_with_a_fresh_rover_home(
        retail_supergraph,
        &[("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "false")],
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|err| panic!("stdout isn't JSON ({err})\n{stderr}"));
    let installed = rover_home.path().join(".rover").join("bin");
    let installed = std::fs::read_dir(&installed).map_or(0, |entries| entries.count());
    assert_eq!(
        (
            output.status.success(),
            json["error"]["code"].as_str(),
            &json["data"]["plugins"],
            installed,
        ),
        (false, Some("E058"), &Value::Array(Vec::new()), 0),
        "{stderr}"
    );
}
