use std::env;
use std::process::Command;
use std::time::Duration;

use assert_cmd::prelude::CommandCargoExt;
use mime::APPLICATION_JSON;
use portpicker::pick_unused_port;
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use rstest::*;
use serde_json::{json, Value};
use speculoos::assert_that;
use tokio::time::timeout;
use tracing::error;
use tracing_test::traced_test;

use super::{
    run_subgraphs_retail_supergraph, test_graphql_connection, RetailSupergraph,
    GRAPHQL_TIMEOUT_DURATION,
};

const ROVER_DEV_TIMEOUT: Duration = Duration::from_secs(45);

#[fixture]
#[once]
fn run_rover_dev(run_subgraphs_retail_supergraph: &RetailSupergraph) -> String {
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    let port = pick_unused_port().expect("No ports free");
    let router_url = format!("http://localhost:{}", port);
    let client = Client::new();

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
    cmd.current_dir(run_subgraphs_retail_supergraph.get_working_directory());
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", version);
    };
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", version);
    };
    #[allow(clippy::zombie_processes)]
    cmd.spawn().expect("Could not run rover dev command");
    tokio::task::block_in_place(|| {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(test_graphql_connection(
            &client,
            &router_url,
            ROVER_DEV_TIMEOUT,
        ))
    })
    .expect("Could not execute check");
    router_url
}

#[rstest]
#[case::simple_subgraph("query {product(id: \"product:2\") { description } }", json!({"data":{"product": {"description": "A classic Supreme vbox t-shirt in the signature Tiffany blue."}}}))]
#[case::multiple_subgraphs("query {order(id: \"order:2\") { items { product { id } inventory { inventory } colorway } buyer { id } } }", json!({"data":{"order":{"items":[{"product":{"id":"product:1"},"inventory":{"inventory":0},"colorway":"Red"}],"buyer":{"id":"user:1"}}}}))]
#[case::deprecated_field("query {product(id: \"product:2\") { reviews { author id } } }", json!({"data":{"product":{"reviews":[{"author":"User 1","id":"review:2"},{"author":"User 1","id":"review:7"}]}}}))]
#[case::deprecated_introspection("query {__type(name:\"Review\"){ fields(includeDeprecated: true) { name isDeprecated deprecationReason } } }", json!({"data":{"__type":{"fields":[{"name":"id","isDeprecated":false,"deprecationReason":null},{"name":"body","isDeprecated":false,"deprecationReason":null},{"name":"author","isDeprecated":true,"deprecationReason":"Use the new `user` field"},{"name":"user","isDeprecated":false,"deprecationReason":null},{"name":"product","isDeprecated":false,"deprecationReason":null}]}}}))]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_dev(
    #[from(run_rover_dev)] router_url: &str,
    #[case] query: String,
    #[case] expected_response: Value,
) {
    let client = Client::new();
    timeout(GRAPHQL_TIMEOUT_DURATION, async {
        loop {
            let req = client
                .post(router_url)
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
                    error!("Error: {}", e)
                }
            };
        }
    })
    .await
    .expect("Failed to run query before timeout hit");
}
