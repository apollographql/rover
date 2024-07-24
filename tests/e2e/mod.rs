use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

use anyhow::Error;
use camino::Utf8PathBuf;
use duct::cmd;
use git2::Repository;
use reqwest::Client;
use rstest::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

mod dev;

const GRAPHQL_TIMEOUT_DURATION: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct ReducedSuperGraphConfig {
    subgraphs: HashMap<String, ReducedSubgraphConfig>,
}
#[derive(Debug, Deserialize)]
struct ReducedSubgraphConfig {
    routing_url: String,
}

impl ReducedSuperGraphConfig {
    pub fn get_subgraph_urls(self) -> Vec<String> {
        self.subgraphs
            .values()
            .map(|x| x.routing_url.clone())
            .collect()
    }
}

#[fixture]
#[once]
fn run_subgraphs_retail_supergraph() -> TempDir {
    println!("Cloning required git repository");
    // Clone the Git Repository that's needed to a temporary folder
    let cloned_dir = TempDir::new().expect("Could not create temporary directory");
    Repository::clone(
        "https://github.com/apollosolutions/retail-supergraph",
        cloned_dir.path(),
    )
    .expect("Could not clone supergraph repository");
    // Jump into that temporary folder and run npm commands to kick off subgraphs
    println!("Installing subgraph dependencies");
    cmd!("npm", "install")
        .dir(cloned_dir.path())
        .run()
        .expect("Could not install subgraph dependencies");
    cmd!("npm", "install", "-g", "nodemon")
        .dir(cloned_dir.path())
        .run()
        .expect("Could not install nodemon");
    println!("Kicking off subgraphs");
    let mut cmd = Command::new("npm");
    cmd.args(["run", "dev:subgraphs"]).current_dir(&cloned_dir);
    cmd.spawn().expect("Could not spawn subgraph process");
    println!("Finding subgraph URLs");
    let subgraph_urls = get_subgraph_urls(
        Utf8PathBuf::from_path_buf(cloned_dir.path().join("supergraph-config-dev.yaml"))
            .expect("Could not create path to config"),
    );
    println!("Testing subgraph connectivity");
    for subgraph_url in subgraph_urls {
        tokio::task::block_in_place(|| {
            let client = Client::new();
            let handle = tokio::runtime::Handle::current();
            handle.block_on(async {
                timeout(
                    GRAPHQL_TIMEOUT_DURATION,
                    test_graphql_connection(&client, &subgraph_url),
                )
                .await
                .expect("Exceeded maximum time allowed")
            })
        })
        .expect("Could not execute connectivity check");
    }
    // Return the folder the subgraphs are in
    cloned_dir
}

async fn test_graphql_connection(client: &Client, url: &str) -> Result<(), Error> {
    let introspection_query = json!({"query": "{__schema{types{name}}}"});
    // Loop until we get a response
    loop {
        match client.post(url).json(&introspection_query).send().await {
            Ok(res) => {
                if res.status().is_success() {
                    break;
                }
            }
            Err(e) => {
                println!(
                    "Could not connect to GraphQL process on {}: {:} - Will retry",
                    url, e
                );
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    println!("Established connection to {}", url);
    Ok(())
}

fn get_subgraph_urls(supergraph_yaml_path: Utf8PathBuf) -> Vec<String> {
    let content = std::fs::read_to_string(supergraph_yaml_path)
        .expect("Could not read supergraph schema file");
    let sc_config: ReducedSuperGraphConfig =
        serde_yaml::from_str(&content).expect("Could not parse supergraph schema file");
    sc_config.get_subgraph_urls()
}
