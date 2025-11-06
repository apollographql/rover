use std::{env, process::Command};

use assert_cmd::prelude::CommandCargoExt;
use regex::RegexSet;
use rstest::*;
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
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("rover");
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
    #[allow(deprecated, reason = "no out of the box spawn command")]
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args(["supergraph", "compose"]);

    // WHEN
    //   - it's invoked
    let res = cmd.spawn().expect("Could not run rover supergraph command");
    let output = res.wait_with_output();

    // THEN
    //   - a failure  code is returned
    assert!(output.is_ok_and(|code| { code.status.code() == Some(2) }));
}
