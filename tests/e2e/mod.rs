use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::Error;
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

mod dev;
mod subgraph;

const GRAPHQL_TIMEOUT_DURATION: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct ReducedSupergraphConfig {
    subgraphs: HashMap<String, ReducedSubgraphConfig>,
}
#[derive(Debug, Deserialize)]
struct ReducedSubgraphConfig {
    routing_url: String,
}

impl ReducedSupergraphConfig {
    pub fn get_subgraph_urls(self) -> Vec<String> {
        self.subgraphs
            .values()
            .map(|x| x.routing_url.clone())
            .collect()
    }
}

const RETAIL_SUPERGRAPH_SCHEMA_NAME: &'static str = "supergraph-config-dev.yaml";

#[fixture]
#[once]
fn run_subgraphs_retail_supergraph() -> TempDir {
    info!("Cloning required git repository");
    // Clone the Git Repository that's needed to a temporary folder
    let cloned_dir = TempDir::new().expect("Could not create temporary directory");
    Repository::clone(
        "https://github.com/apollosolutions/retail-supergraph",
        cloned_dir.path(),
    )
    .expect("Could not clone supergraph repository");
    // Jump into that temporary folder and run npm commands to kick off subgraphs
    info!("Installing subgraph dependencies");
    cmd!("npm", "install")
        .dir(cloned_dir.path())
        .run()
        .expect("Could not install subgraph dependencies");
    cmd!("npm", "install", "-g", "nodemon")
        .dir(cloned_dir.path())
        .run()
        .expect("Could not install nodemon");
    info!("Kicking off subgraphs");
    let mut cmd = Command::new("npx");
    cmd.env("NODE_ENV", "dev");
    cmd.args(["nodemon", "index.js"]).current_dir(&cloned_dir);
    cmd.spawn().expect("Could not spawn subgraph process");
    info!("Finding subgraph URLs");
    let subgraph_urls =
        get_supergraph_config(cloned_dir.path().join(RETAIL_SUPERGRAPH_SCHEMA_NAME))
            .get_subgraph_urls();
    info!("Testing subgraph connectivity");
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
    // Return the folder the subgraphs are in
    cloned_dir
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
        .dir(&target.path())
        .run()
        .expect("Could not clean directory");
    cmd!("npm", "install")
        .dir(&target.path())
        .run()
        .expect("Could not install subgraph dependencies");
    info!("Kicking off subgraphs");
    let mut cmd = Command::new("npm");
    let port = pick_unused_port().expect("No free ports");
    let url = format!("http://localhost:{}", port);
    cmd.args(["run", "start", "--", &port.to_string()])
        .current_dir(&target.path());
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

fn get_supergraph_config(supergraph_yaml_path: PathBuf) -> ReducedSupergraphConfig {
    let content = std::fs::read_to_string(supergraph_yaml_path)
        .expect("Could not read supergraph schema file");
    serde_yaml::from_str(&content).expect("Could not parse supergraph schema file")
}
