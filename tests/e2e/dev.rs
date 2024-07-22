use std::env;
use std::path::Path;
use std::process::Child;
use std::process::Command;

use assert_cmd::prelude::CommandCargoExt;
use mime::APPLICATION_JSON;
use rand::Rng;
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use rstest::*;
use serde_json::{json, Value};
use speculoos::assert_that;
use tempfile::TempDir;
use tokio::time::timeout;

use crate::e2e::{
    run_subgraphs_retail_supergraph, test_graphql_connection, GRAPHQL_TIMEOUT_DURATION,
};

async fn run_rover_dev(client: &Client, working_directory: &Path) -> (Child, String) {
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    let mut rng = rand::thread_rng();
    let port = rng.gen_range(4002..4050);
    let router_url = format!("http://localhost:{}", port);
    cmd.args([
        "dev",
        "--supergraph-config",
        "supergraph-config-dev.yaml",
        "--router-config",
        "router-config-dev.yaml",
        "--supergraph-port",
        &format!("{}", port),
        "--elv2-license",
        "accept",
    ]);
    cmd.current_dir(working_directory);
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", version);
    };
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", version);
    };
    let handle = cmd.spawn().expect("Could not run rover dev command");
    timeout(
        GRAPHQL_TIMEOUT_DURATION,
        test_graphql_connection(&client, &router_url),
    )
    .await
    .expect("Could not execute check")
    .expect("foo");
    (handle, router_url)
}

#[rstest]
#[case::simple_subgraph("query {product(id: \"product:2\") { description } }", json!({"data":{"product": {"description": "A classic Supreme vbox t-shirt in the signature Tiffany blue."}}}))]
#[case::multiple_subgraphs("query {order(id: \"order:2\") { items { product { id } inventory { inventory } colorway } buyer { id } } }", json!({"data":{"order":{"items":[{"product":{"id":"product:1"},"inventory":{"inventory":0},"colorway":"Red"}],"buyer":{"id":"user:1"}}}}))]
#[case::deprecated_field("query {product(id: \"product:2\") { reviews { author id } } }", json!({"data":{"product":{"reviews":[{"author":"User 1","id":"review:2"},{"author":"User 1","id":"review:7"}]}}}))]
#[case::deprecated_introspection("query {__type(name:\"Review\"){ fields(includeDeprecated: true) { name isDeprecated deprecationReason } } }", json!({"data":{"__type":{"fields":[{"name":"id","isDeprecated":false,"deprecationReason":null},{"name":"body","isDeprecated":false,"deprecationReason":null},{"name":"author","isDeprecated":true,"deprecationReason":"Use the new `user` field"},{"name":"user","isDeprecated":false,"deprecationReason":null},{"name":"product","isDeprecated":false,"deprecationReason":null}]}}}))]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_rover_dev(
    run_subgraphs_retail_supergraph: &(Child, TempDir),
    #[case] query: String,
    #[case] expected_response: Value,
) {
    let client = Client::new();
    let (mut handle, url) = run_rover_dev(&client, run_subgraphs_retail_supergraph.1.path()).await;
    timeout(GRAPHQL_TIMEOUT_DURATION, async {
        loop {
            let req = client
                .post(&url)
                .header(CONTENT_TYPE, APPLICATION_JSON.to_string())
                .json(&json!({"query": query}))
                .send();
            match req.await {
                Ok(value) => {
                    let actual_response: Value =
                        value.json().await.expect("Could not get response");
                    assert_that!(&actual_response).is_equal_to(expected_response.clone());
                    break;
                }
                Err(e) => {
                    println!("Error: {}", e)
                }
            };
        }
    })
    .await
    .expect("Failed to run query before timeout hit");
    handle.kill().unwrap();
}
