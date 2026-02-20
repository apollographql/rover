use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::ChildStderr,
    time::Duration,
};

use anyhow::Error;
use camino::Utf8PathBuf;
use dircpy::CopyBuilder;
use itertools::Itertools;
use portpicker::pick_unused_port;
use regex::Regex;
use reqwest::Client;
use rover::utils::template::download_template;
use rstest::*;
use serde::Deserialize;
use serde_json::json;
use subgraph_mock::state::{Config, State};
use tempfile::TempDir;
use tokio::{
    runtime::Runtime,
    task::{AbortHandle, JoinHandle},
    time::timeout,
};
use tracing::{info, warn};

mod config;
mod dev;
mod graph;
mod init;
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
pub struct RetailSupergraph {
    retail_supergraph_config: RetailSupergraphConfig,
    working_dir: TempDir,
}

pub struct RunningRetailSupergraph {
    pub retail_supergraph: &'static RetailSupergraph,
    subgraph_handles: Vec<AbortHandle>,
}

impl Drop for RunningRetailSupergraph {
    fn drop(&mut self) {
        for handle in &self.subgraph_handles {
            handle.abort();
        }
    }
}

#[derive(Debug, Deserialize)]
struct ReducedSubgraphConfig {
    routing_url: String,
    schema: ReducedSchemaLocation,
}

#[derive(Debug, Deserialize)]
struct ReducedSchemaLocation {
    file: PathBuf,
}

impl RetailSupergraph {
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
}

fn clone_retail_supergraph_repo() -> TempDir {
    info!("Cloning required git repository");
    // Clone the Git Repository that's needed to a temporary folder
    let working_dir = TempDir::new().expect("Could not create temporary directory");

    // Run this one-off with Tokio rather than make all these tests async
    tokio::task::block_in_place(|| {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(async {
            download_template(
                "https://github.com/apollosolutions/retail-supergraph/archive/refs/heads/main.tar.gz"
                    .parse()
                    .unwrap(),
                &Utf8PathBuf::from_path_buf(working_dir.path().to_path_buf()).unwrap(),
            )
            .await
            .expect("Could not download supergraph repository");
        });
    });

    working_dir
}

#[fixture]
#[once]
/// Tokio runtime that will outlive any given test execution, so that background tasks such as the subgraph
/// mocks can persist across them.
///
/// This isn't needed if all tests are passing, but any failing test will terminate
/// the test process' tokio runtime (due to test failures panicking their thread),
/// which then cascades to every other test that expects these tasks to still be running failing as well.
fn background_runtime() -> Runtime {
    Runtime::new().expect("Task runtime for subgraphs should be created")
}

#[fixture]
#[once]
fn run_subgraphs_retail_supergraph(
    retail_supergraph: &'static RetailSupergraph,
    background_runtime: &'static Runtime,
) -> RunningRetailSupergraph {
    println!("Kicking off subgraphs");

    let subgraph_configs = &retail_supergraph.retail_supergraph_config.subgraphs;

    let default_mock_config = {
        let mut default = Config::default();
        // Don't generate null/0-length values so our tests can make assertions on the shape of responses
        default.response_generation.null_ratio = None;
        default.response_generation.array.min_length = 1;
        default
    };

    let subgraph_handles: Vec<_> = subgraph_configs
        .values()
        .map(|subgraph_config| {
            let port = subgraph_config
                .routing_url
                .split(":")
                .last()
                .and_then(|substr| substr.split("/").next())
                .and_then(|port| port.parse().ok())
                .expect("failed to extract the port from the routing URL");

            background_runtime
                .spawn(subgraph_mock::mock_server_loop(
                    port,
                    State::new(
                        default_mock_config.clone(),
                        retail_supergraph
                            .working_dir
                            .path()
                            .join(&subgraph_config.schema.file),
                    )
                    .expect("Failed to parse retail subgraph schema"),
                ))
                .abort_handle()
        })
        .collect();

    println!("Testing subgraph connectivity");
    for subgraph_config in subgraph_configs.values() {
        tokio::task::block_in_place(|| {
            let client = Client::new();
            let handle = tokio::runtime::Handle::current();
            handle.block_on(test_graphql_connection(
                &client,
                &subgraph_config.routing_url,
                GRAPHQL_TIMEOUT_DURATION,
            ))
        })
        .expect("Could not execute connectivity check");
    }
    RunningRetailSupergraph {
        retail_supergraph,
        subgraph_handles,
    }
}

#[fixture]
#[once]
fn retail_supergraph() -> RetailSupergraph {
    let working_dir = clone_retail_supergraph_repo();

    let supergraph_yaml_path = working_dir.path().join("supergraph-config-dev.yaml");
    let content = std::fs::read_to_string(&supergraph_yaml_path)
        .expect("Could not read supergraph schema file");

    // Rewrite the subgraph URLs to have each subgraph running on a different port
    let base_port = 4001; // as defined in supergraph-config-dev.yaml
    let base_port_stringified = base_port.to_string();

    let contents = content.split(&base_port_stringified);
    let subgraph_count = contents.clone().count() as u16 - 1;
    let ports: Vec<u16> = (base_port..(base_port + subgraph_count)).collect();

    let content: String = contents
        .map(ToOwned::to_owned)
        .interleave(ports.iter().map(ToString::to_string))
        .collect();

    let retail_supergraph_config: RetailSupergraphConfig =
        serde_yaml::from_str(&content).expect("Could not parse supergraph schema file");

    std::fs::write(supergraph_yaml_path, content)
        .expect("Could not rewrite supergraph schema file");

    RetailSupergraph {
        retail_supergraph_config,
        working_dir,
    }
}

struct SingleMutableSubgraph {
    subgraph_url: String,
    directory: TempDir,
    schema_file_name: String,
    task_handle: JoinHandle<Result<(), Error>>,
}

impl Drop for SingleMutableSubgraph {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

#[fixture]
async fn run_single_mutable_subgraph(test_artifacts_directory: PathBuf) -> SingleMutableSubgraph {
    // Create a copy of one of the subgraphs in a temporary subfolder
    let target = TempDir::new().expect("Could not create temporary directory");
    CopyBuilder::new(test_artifacts_directory.join("pandas"), &target)
        .with_include_filter(".")
        .run()
        .expect("Could not perform copy");

    let port = pick_unused_port().expect("No free ports");
    let subgraph_url = format!("http://localhost:{port}");
    let task_handle = tokio::spawn(subgraph_mock::mock_server_loop(
        port,
        State::default(target.path().join("pandas.graphql"))
            .expect("Failed to parse pandas.graphql"),
    ));

    info!("Testing subgraph connectivity");
    let client = Client::new();
    test_graphql_connection(&client, &subgraph_url, GRAPHQL_TIMEOUT_DURATION)
        .await
        .expect("Could not execute connectivity check");
    SingleMutableSubgraph {
        subgraph_url,
        directory: target,
        schema_file_name: String::from("pandas.graphql"),
        task_handle,
    }
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

fn find_matching_log_line(reader: &mut BufReader<ChildStderr>, matcher: &Regex) {
    info!("Waiting for matching log line...");
    let mut introspection_line = String::new();
    loop {
        reader
            .read_line(&mut introspection_line)
            .expect("Could not read line from console process");
        info!("Line read from spawned process '{introspection_line}'");
        if matcher.is_match(&introspection_line) {
            break;
        } else {
            introspection_line.clear();
        }
    }
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

#[fixture]
#[once]
fn introspection_log_line_prefix() -> Regex {
    Regex::new("Introspection Response").unwrap()
}
