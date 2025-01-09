use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{self, Command, Stdio};
use std::time::Duration;
use std::{env, thread::sleep};

use assert_cmd::output::OutputOkExt;
use assert_cmd::prelude::CommandCargoExt;
use mime::APPLICATION_JSON;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use portpicker::pick_unused_port;
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use rstest::*;
use serde_json::{json, Value};
use serial_test::serial;
use speculoos::assert_that;
use tempfile::TempDir;
use tokio::time::timeout;
use tracing::error;
use tracing_test::traced_test;

use super::{
    run_subgraphs_retail_supergraph, test_graphql_connection, RetailSupergraph,
    GRAPHQL_TIMEOUT_DURATION,
};

const ROVER_DEV_TIMEOUT: Duration = Duration::from_secs(45);

//#[fixture]
//#[once]
//fn run_rover_dev(run_subgraphs_retail_supergraph: &RetailSupergraph) -> String {
//    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
//    let port = pick_unused_port().expect("No ports free");
//    let router_url = format!("http://localhost:{}", port);
//    let client = Client::new();
//
//    cmd.args([
//        "dev",
//        "--supergraph-config",
//        "supergraph-config-dev.yaml",
//        "--router-config",
//        "router-config-dev.yaml",
//        "--supergraph-port",
//        &format!("{}", port),
//        "--elv2-license",
//        "accept",
//        "--log",
//        "debug",
//    ])
//    .stderr(Stdio::piped())
//    .stdout(Stdio::piped());
//
//    cmd.current_dir(run_subgraphs_retail_supergraph.get_working_directory());
//    if let Ok(version) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
//        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", version);
//    };
//    if let Ok(version) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
//        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", version);
//    };
//
//    println!("here!");
//    let mut child = cmd.spawn().expect("Could not run rover dev command");
//    let stdout = child.stdout.take().unwrap();
//    let stderr = child.stderr.take().unwrap();
//
//    tokio::spawn(async {
//        timeout(Duration::from_secs(15), async {
//            let reader = BufReader::new(stdout);
//            for line in reader.lines() {
//                if let Ok(line) = line {
//                    println!("stdout: {line}");
//                }
//            }
//
//            let reader = BufReader::new(stderr);
//            for line in reader.lines() {
//                if let Ok(line) = line {
//                    println!("stderr: {line}");
//                }
//            }
//        })
//        //task::yield_now();
//    });
//
//    tokio::task::block_in_place(|| {
//        let handle = tokio::runtime::Handle::current();
//        handle.block_on(test_graphql_connection(
//            &client,
//            &router_url,
//            ROVER_DEV_TIMEOUT,
//        ))
//    })
//    .expect("Could not execute check");
//    router_url
//}

//fn log_lines(reader: &mut BufReader<ChildStderr>, matcher: &Regex) {
//    loop {
//        reader
//            .read_line(&mut introspection_line)
//            .expect("Could not read line from console process");
//        info!("Line read from spawned process '{introspection_line}'");
//        if matcher.is_match(&introspection_line) {
//            break;
//        } else {
//            introspection_line.clear();
//        }
//    }
//}
//
//

#[fixture]
#[once]
fn retail_supergraph_dir(run_subgraphs_retail_supergraph: &RetailSupergraph) -> String {
    run_subgraphs_retail_supergraph
        .get_working_directory()
        .path()
        .to_str()
        .unwrap()
        .to_string()
}

//#[rstest]
////#[case::simple_subgraph("query {product(id: \"product:2\") { description } }", json!({"data":{"product": {"description": "A classic Supreme vbox t-shirt in the signature Tiffany blue."}}}))]
//#[case::multiple_subgraphs("query {order(id: \"order:2\") { items { product { id } inventory { inventory } colorway } buyer { id } } }", json!({"data":{"order":{"items":[{"product":{"id":"product:1"},"inventory":{"inventory":0},"colorway":"Red"}],"buyer":{"id":"user:1"}}}}))]
////#[case::deprecated_field("query {product(id: \"product:2\") { reviews { author id } } }", json!({"data":{"product":{"reviews":[{"author":"User 1","id":"review:2"},{"author":"User 1","id":"review:7"}]}}}))]
////#[case::deprecated_introspection("query {__type(name:\"Review\"){ fields(includeDeprecated: true) { name isDeprecated deprecationReason } } }", json!({"data":{"__type":{"fields":[{"name":"id","isDeprecated":false,"deprecationReason":null},{"name":"body","isDeprecated":false,"deprecationReason":null},{"name":"author","isDeprecated":true,"deprecationReason":"Use the new `user` field"},{"name":"user","isDeprecated":false,"deprecationReason":null},{"name":"product","isDeprecated":false,"deprecationReason":null}]}}}))]
//#[ignore]
////#[tokio::test(flavor = "current_thread")]
//#[tokio::test(flavor = "multi_thread")]
////#[tokio::test:]
//#[traced_test]
//async fn e2e_test_rover_dev(
//    retail_supergraph_dir: &String,
//    //run_subgraphs_retail_supergraph: &RetailSupergraph<'_>,
//    //#[from(run_rover_dev)] router_url: &str,
//    #[case] query: String,
//    #[case] expected_response: Value,
//) {
//    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
//    let port = pick_unused_port().expect("No ports free");
//    let router_url = format!("http://localhost:{}", port);
//    let client = Client::new();
//
//    cmd.args([
//        "dev",
//        "--supergraph-config",
//        "supergraph-config-dev.yaml",
//        "--router-config",
//        "router-config-dev.yaml",
//        "--supergraph-port",
//        &format!("{}", port),
//        "--elv2-license",
//        "accept",
//        "--log",
//        "debug",
//    ])
//    .stderr(Stdio::piped())
//    .stdout(Stdio::piped());
//
//    //cmd.current_dir(run_subgraphs_retail_supergraph.get_working_directory());
//    cmd.current_dir(retail_supergraph_dir);
//    if let Ok(version) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
//        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", version);
//    };
//    if let Ok(version) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
//        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", version);
//    };
//
//    println!("here!");
//    let mut child = cmd.spawn().expect("Could not run rover dev command");
//    let stdout = child.stdout.take().unwrap();
//    let stderr = child.stderr.take().unwrap();
//
//    tokio::spawn(async {
//        //timeout(Duration::from_secs(15), async {
//        let reader = BufReader::new(stdout);
//        for line in reader.lines() {
//            if let Ok(line) = line {
//                println!("stdout: {line}");
//            }
//        }
//
//        let reader = BufReader::new(stderr);
//        for line in reader.lines() {
//            if let Ok(line) = line {
//                println!("stderr: {line}");
//            }
//        }
//        //})
//        //task::yield_now();
//    });
//
//    tokio::task::block_in_place(|| {
//        let handle = tokio::runtime::Handle::current();
//        handle.block_on(test_graphql_connection(
//            &client,
//            &router_url,
//            ROVER_DEV_TIMEOUT,
//        ))
//    })
//    .expect("Could not execute check");
//    //router_url
//
//    let client = Client::new();
//    //let _ = timeout(GRAPHQL_TIMEOUT_DURATION, async {
//    //loop {
//    println!("hello?");
//    let req = client
//        .post(router_url.clone())
//        .header(CONTENT_TYPE, APPLICATION_JSON.to_string())
//        .json(&json!({"query": query}))
//        .send();
//
//    match req.await {
//        Ok(value) => {
//            println!("in ok(val) for req");
//            let actual_response: Value = value.json().await.expect("Could not get response");
//            println!("actual response: {actual_response:?}");
//            assert_that!(&actual_response).is_equal_to(expected_response.clone());
//            //break;
//        }
//        Err(e) => {
//            println!("in err(e) for req: {e}");
//            error!("Error: {}", e)
//        }
//    };
//    //}
//    //})
//    //.await;
//    //.expect("Failed to run query before timeout hit");
//    //
//
//    signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGINT).unwrap();
//    tokio::time::sleep(Duration::from_secs(3)).await;
//    signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGINT).unwrap();
//
//    //let _ = timeout(GRAPHQL_TIMEOUT_DURATION, async {
//    //child.kill().expect("Failed to kill rover dev process");
//    //});
//}

//test e2e::dev::e2e_test_rover_dev::case_1_simple_subgraph ... ok
//test e2e::dev::blah_e2e_test_rover_dev::case_1_multiple_subgraphs ... ok

#[rstest]
//#[case::simple_subgraph("query {product(id: \"product:2\") { description } }", json!({"data":{"product": {"description": "A classic Supreme vbox t-shirt in the signature Tiffany blue."}}}))]
//#[case::multiple_subgraphs("query {order(id: \"order:2\") { items { product { id } inventory { inventory } colorway } buyer { id } } }", json!({"data":{"order":{"items":[{"product":{"id":"product:1"},"inventory":{"inventory":0},"colorway":"Red"}],"buyer":{"id":"user:1"}}}}))]
//#[case::deprecated_field("query {product(id: \"product:2\") { reviews { author id } } }", json!({"data":{"product":{"reviews":[{"author":"User 1","id":"review:2"},{"author":"User 1","id":"review:7"}]}}}))]
//#[case::deprecated_introspection("query {__type(name:\"Review\"){ fields(includeDeprecated: true) { name isDeprecated deprecationReason } } }", json!({"data":{"__type":{"fields":[{"name":"id","isDeprecated":false,"deprecationReason":null},{"name":"body","isDeprecated":false,"deprecationReason":null},{"name":"author","isDeprecated":true,"deprecationReason":"Use the new `user` field"},{"name":"user","isDeprecated":false,"deprecationReason":null},{"name":"product","isDeprecated":false,"deprecationReason":null}]}}}))]
#[ignore]
//#[tokio::test(flavor = "current_thread")]
#[tokio::test(flavor = "multi_thread")]
//#[tokio::test:]
#[traced_test]
async fn e2e_test_rover_dev(
    retail_supergraph_dir: &String,
    //run_subgraphs_retail_supergraph: &RetailSupergraph<'_>,
    //#[from(run_rover_dev)] router_url: &str,
    //#[case] query: String,
    //#[case] expected_response: Value,
) {
    let test_cases = vec![
    ("simple subgraph", "query {product(id: \"product:2\") { description } }", json!({"data":{"product": {"description": "A classic Supreme vbox t-shirt in the signature Tiffany blue."}}})),
    ("multiple subgraphs", "query {order(id: \"order:2\") { items { product { id } inventory { inventory } colorway } buyer { id } } }", json!({"data":{"order":{"items":[{"product":{"id":"product:1"},"inventory":{"inventory":0},"colorway":"Red"}],"buyer":{"id":"user:1"}}}})),

    ("deprecated field", "query {product(id: \"product:2\") { reviews { author id } } }", json!({"data":{"product":{"reviews":[{"author":"User 1","id":"review:2"},{"author":"User 1","id":"review:7"}]}}})),
    ("deprecated introspection", "query {__type(name:\"Review\"){ fields(includeDeprecated: true) { name isDeprecated deprecationReason } } }", json!({"data":{"__type":{"fields":[{"name":"id","isDeprecated":false,"deprecationReason":null},{"name":"body","isDeprecated":false,"deprecationReason":null},{"name":"author","isDeprecated":true,"deprecationReason":"Use the new `user` field"},{"name":"user","isDeprecated":false,"deprecationReason":null},{"name":"product","isDeprecated":false,"deprecationReason":null}]}}}))];

    let port = pick_unused_port().expect("No ports free");
    let router_url = format!("http://localhost:{}", port);
    let client = Client::new();
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");

    cmd.args(["config", "whoami"]);

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
        "--log",
        "debug",
    ])
    .stderr(Stdio::piped())
    .stdout(Stdio::piped());

    //cmd.current_dir(run_subgraphs_retail_supergraph.get_working_directory());
    cmd.current_dir(retail_supergraph_dir);
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", version);
    };
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", version);
    };

    println!("here!");
    let mut child = cmd.spawn().expect("Could not run rover dev command");
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    tokio::spawn(async {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("stdout: {line}");
            }
        }

        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("stderr: {line}");
            }
        }
    });

    for (name, query, expectation) in test_cases {
        println!("test case: {name}");
        tokio::task::block_in_place(|| {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(test_graphql_connection(
                &client,
                &router_url,
                ROVER_DEV_TIMEOUT,
            ))
        })
        .expect("Could not execute check");

        let client = Client::new();
        let req = client
            .post(router_url.clone())
            .header(CONTENT_TYPE, APPLICATION_JSON.to_string())
            .json(&json!({"query": query}))
            .send();

        match req.await {
            Ok(value) => {
                println!("in ok(val) for req");
                let actual_response: Value = value.json().await.expect("Could not get response");
                println!("actual response: {actual_response:?}");
                assert_that!(&actual_response).is_equal_to(expectation.clone());
                //break;
            }
            Err(e) => {
                println!("in err(e) for req: {e}");
                error!("Error: {}", e)
            }
        };

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGINT).unwrap();
}
