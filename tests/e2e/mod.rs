use std::env;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::{collections::HashMap, path::PathBuf};

use anyhow::Error;
use camino::Utf8PathBuf;
use dircpy::CopyBuilder;
use duct::cmd;
use git2::Repository;
use portpicker::pick_unused_port;
use reqwest::Client;
use rstest::*;
use serde::Deserialize;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use tracing::{info, warn};

mod config;
mod dev;
mod graph;
mod install;
mod options;
mod subgraph;
mod supergraph;

const GRAPHQL_TIMEOUT_DURATION: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
pub struct RetailSupergraphConfig {
    subgraphs: HashMap<String, ReducedSubgraphConfig>,
}

#[derive(Debug)]
pub struct RetailSupergraph<'a> {
    retail_supergraph_config: RetailSupergraphConfig,
    working_dir: Option<&'a TempDir>,
}

#[derive(Debug, Deserialize)]
struct ReducedSubgraphConfig {
    routing_url: String,
}

impl RetailSupergraph<'_> {
    pub fn get_subgraph_urls(&self) -> Vec<String> {
        self.retail_supergraph_config
            .subgraphs
            .values()
            .map(|x| x.routing_url.clone())
            .collect()
    }

    pub fn get_subgraph_names(&self) -> Vec<String> {
        self.retail_supergraph_config
            .subgraphs
            .keys()
            .cloned()
            .collect()
    }

    pub fn get_working_directory(&self) -> &TempDir {
        self.working_dir.expect("no working directory")
    }
}

#[fixture]
#[once]
fn clone_retail_supergraph_repo() -> TempDir {
    info!("Cloning required git repository");
    // Clone the Git Repository that's needed to a temporary folder
    let working_dir = TempDir::new().expect("Could not create temporary directory");
    Repository::clone(
        "https://github.com/apollosolutions/retail-supergraph",
        working_dir.path(),
    )
    .expect("Could not clone supergraph repository");

    working_dir
}

#[fixture]
#[once]
fn run_subgraphs_retail_supergraph(
    retail_supergraph: &'static RetailSupergraph,
) -> &'static RetailSupergraph<'static> {
    println!("Kicking off subgraphs");

    // Although the retail supergraph package.json has a `dev:subgraphs` script, windows can't
    // recognize the `NODE_ENV=dev` preprended variable; so, we have to remake that command in a
    // way that windows can understand
    let mut cmd = Command::new("npx");
    cmd.env("NODE_ENV", "dev");
    cmd.args(["nodemon", "index.js"])
        .current_dir(retail_supergraph.get_working_directory());
    cmd.spawn().expect("Could not spawn subgraph process");

    println!("Finding subgraph URLs");
    let subgraph_urls = retail_supergraph.get_subgraph_urls();

    println!("Testing subgraph connectivity");
    for subgraph_url in subgraph_urls {
        tokio::task::block_in_place(|| {
            let client = Client::new();
            let handle = tokio::runtime::Handle::current();
            handle.block_on(test_graphql_connection(
                &client,
                &subgraph_url,
                GRAPHQL_TIMEOUT_DURATION,
            ))
        })
        .expect("Could not execute connectivity check");
    }
    retail_supergraph
}

#[fixture]
#[once]
fn retail_supergraph(clone_retail_supergraph_repo: &'static TempDir) -> RetailSupergraph<'static> {
    // Jump into that temporary folder and run npm commands to kick off subgraphs
    info!("Installing subgraph dependencies");
    cmd!("npm", "install")
        .dir(clone_retail_supergraph_repo.path())
        .run()
        .expect("Could not install subgraph dependencies");

    let supergraph_yaml_path = Utf8PathBuf::from_path_buf(
        clone_retail_supergraph_repo
            .path()
            .join("supergraph-config-dev.yaml"),
    )
    .expect("Could not create path to config");

    let content = std::fs::read_to_string(supergraph_yaml_path)
        .expect("Could not read supergraph schema file");

    let retail_supergraph_config: RetailSupergraphConfig =
        serde_yaml::from_str(&content).expect("Could not parse supergraph schema file");

    RetailSupergraph {
        retail_supergraph_config,
        working_dir: Some(clone_retail_supergraph_repo),
    }
}

#[fixture]
async fn run_single_mutable_subgraph() -> (String, TempDir, String) {
    // Create a copy of one of the subgraphs in a temporary subfolder
    let target = TempDir::new().expect("Could not create temporary directory");
    let cargo_manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("Could not find CARGO_MANIFEST_DIR");
    CopyBuilder::new(
        Path::new(&cargo_manifest_dir).join("examples/supergraph-demo/pandas"),
        &target,
    )
    .with_include_filter(".")
    .run()
    .expect("Could not perform copy");

    info!("Installing subgraph dependencies");
    cmd!("npm", "run", "clean")
        .dir(target.path())
        .run()
        .expect("Could not clean directory");
    cmd!("npm", "install")
        .dir(target.path())
        .run()
        .expect("Could not install subgraph dependencies");
    info!("Kicking off subgraphs");
    let mut cmd = Command::new("npm");
    let port = pick_unused_port().expect("No free ports");
    let url = format!("http://localhost:{}", port);
    cmd.args(["run", "start", "--", &port.to_string()])
        .current_dir(target.path());
    cmd.spawn().expect("Could not spawn subgraph process");
    info!("Testing subgraph connectivity");
    let client = Client::new();
    test_graphql_connection(&client, &url, GRAPHQL_TIMEOUT_DURATION)
        .await
        .expect("Could not execute connectivity check");
    (url, target, String::from("pandas.graphql"))
}

async fn test_graphql_connection(
    client: &Client,
    url: &str,
    timeout_duration: Duration,
) -> Result<(), Error> {
    let introspection_query = json!({"query": "{__schema{types{name}}}"});
    // Loop until we get a response, but timeout if it takes too long
    timeout(timeout_duration, async {
        loop {
            match client.post(url).json(&introspection_query).send().await {
                Ok(res) => {
                    if res.status().is_success() {
                        break;
                    }
                }
                Err(e) => {
                    warn!(
                        "Could not connect to GraphQL process on {}: {:} - Will retry",
                        url, e
                    );
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    })
    .await?;
    info!("Established connection to {}", url);
    Ok(())
}

#[fixture]
fn remote_supergraph_graphref() -> String {
    String::from("rover-e2e-tests@current")
}

#[fixture]
fn remote_supergraph_publish_test_variant_graphref() -> String {
    String::from("rover-e2e-tests@publish-test")
}
#[fixture]
fn test_artifacts_directory() -> PathBuf {
    let cargo_manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("Could not find CARGO_MANIFEST_DIR");
    PathBuf::from(cargo_manifest_dir).join("tests/e2e/artifacts")
}
