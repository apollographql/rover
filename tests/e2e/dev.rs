use std::{env, process::Command, time::Duration};

use assert_cmd::cargo;
use json_matcher::{
    AnyMatcher, JsonMatcher, JsonMatcherError, JsonPath, JsonPathElement, ObjectMatcher, assert_jm,
};
use mime::APPLICATION_JSON;
use portpicker::pick_unused_port;
use reqwest::{Client, header::CONTENT_TYPE};
use rstest::*;
use serde_json::{Value, json};
use speculoos::assert_that;
use tokio::time::timeout;
use tracing::error;
use tracing_test::traced_test;

use super::{
    GRAPHQL_TIMEOUT_DURATION, RunningRetailSupergraph, run_subgraphs_retail_supergraph,
    test_graphql_connection,
};

const ROVER_DEV_TIMEOUT: Duration = Duration::from_secs(45);

#[fixture]
#[once]
#[allow(clippy::zombie_processes)]
fn run_rover_dev(run_subgraphs_retail_supergraph: &RunningRetailSupergraph) -> String {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    let port = pick_unused_port().expect("No ports free");
    let router_url = format!("http://localhost:{port}");
    let client = Client::new();

    cmd.args([
        "dev",
        "--supergraph-config",
        "supergraph-config-dev.yaml",
        "--router-config",
        "router-config-dev.yaml",
        "--supergraph-port",
        &format!("{port}"),
        "--elv2-license",
        "accept",
    ]);
    cmd.current_dir(
        &run_subgraphs_retail_supergraph
            .retail_supergraph
            .working_dir,
    );
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", version);
    };
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", version);
    };
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_MCP_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_MCP_VERSION", version);
    };
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

/// The default string matcher expects a particular value
struct NonNullString;
impl JsonMatcher for NonNullString {
    fn json_matches(&self, value: &Value) -> Vec<JsonMatcherError> {
        match value.as_str() {
            Some(_) => vec![],
            None => vec![JsonMatcherError::at_root("Expected string")],
        }
    }
}
impl NonNullString {
    fn boxed() -> Box<dyn JsonMatcher> {
        Box::new(Self)
    }
}

/// The default number matcher expects a particular value
struct NonNullNumber;
impl JsonMatcher for NonNullNumber {
    fn json_matches(&self, value: &Value) -> Vec<JsonMatcherError> {
        match value.as_number() {
            Some(_) => vec![],
            None => vec![JsonMatcherError::at_root("Expected number")],
        }
    }
}
impl NonNullNumber {
    fn boxed() -> Box<dyn JsonMatcher> {
        Box::new(Self)
    }
}

/// The default array matcher expects a particular length
struct AnyLengthArray(Box<dyn JsonMatcher>);
impl JsonMatcher for AnyLengthArray {
    fn json_matches(&self, value: &Value) -> Vec<JsonMatcherError> {
        match value.as_array() {
            Some(arr) => arr
                .iter()
                .enumerate()
                .flat_map(|(index, element)| {
                    self.0.json_matches(element).into_iter().map(move |error| {
                        let this_path = JsonPath::from(vec![
                            JsonPathElement::Root,
                            JsonPathElement::Index(index),
                        ]);
                        let JsonMatcherError { path, message } = error;
                        let new_path = this_path.extend(path);
                        JsonMatcherError {
                            path: new_path,
                            message,
                        }
                    })
                })
                .collect(),
            None => vec![JsonMatcherError::at_root("Expected array")],
        }
    }
}

#[rstest]
#[case::simple_subgraph(
    "query {product(id: \"product:2\") { description } }", 
    |val| assert_jm!(val, { "data": { "product": { "description": NonNullString }}})
)]
#[case::multiple_subgraphs(
    "query {order(id: \"order:2\") { items { product { id } inventory { inventory } colorway } buyer { id } } }", 
    |val| assert_jm!(val, {
        "data": {
            "order": {
                // Because the subgraph mocks return random data, these aren't guaranteed to be non-null when the
                // router joins the results together
                "items": AnyLengthArray(Box::new(ObjectMatcher::of(vec![
                    ("product".to_string(), Box::new(AnyMatcher::new()) as Box<dyn JsonMatcher>),
                    ("inventory".to_string(), Box::new(AnyMatcher::new()) as Box<dyn JsonMatcher>),
                    ("colorway".to_string(), Box::new(AnyMatcher::new()) as Box<dyn JsonMatcher>)
                ].into_iter().collect()))),
                "buyer": ObjectMatcher::of(vec![("id".to_string(), NonNullNumber::boxed())].into_iter().collect())
            }}})
)]
#[case::deprecated_field(
    "query {product(id: \"product:2\") { reviews { author id } } }", 
    |val| assert_jm!(val, {
        "data": {
            "product": {
                "reviews": AnyLengthArray(Box::new(ObjectMatcher::of(vec![
                    ("author".to_string(), NonNullString::boxed()),
                    ("id".to_string(), NonNullNumber::boxed())
                ].into_iter().collect())))
            }
        }
    })
)]
#[case::deprecated_introspection(
    "query {__type(name:\"Review\"){ fields(includeDeprecated: true) { name isDeprecated deprecationReason } } }",
    |val| assert_that(&val).is_equal_to(json!(
        {
            "data":{
                "__type":{
                    "fields":[
                        {"name":"id","isDeprecated":false,"deprecationReason":null},
                        {"name":"body","isDeprecated":false,"deprecationReason":null},
                        {"name":"author","isDeprecated":true,"deprecationReason":"Use the new `user` field"},
                        {"name":"user","isDeprecated":false,"deprecationReason":null},
                        {"name":"product","isDeprecated":false,"deprecationReason":null}
                    ]
                }
            }
        })))]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_dev(
    #[from(run_rover_dev)] router_url: &str,
    #[case] query: String,
    #[case] assertion: impl FnOnce(Value),
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
                    assertion(actual_response);
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
