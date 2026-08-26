use std::{
    collections::VecDeque,
    env,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use assert_cmd::cargo;
use json_matcher::{
    AnyMatcher, JsonMatcher, JsonMatcherError, JsonPath, JsonPathElement, ObjectMatcher, assert_jm,
};
use mime::APPLICATION_JSON;
use reqwest::{Client, header::CONTENT_TYPE};
use rstest::*;
use serde_json::{Value, json};
use serial_test::serial;
use speculoos::{assert_that, result::ResultAssertions, string::StrAssertions};
use tempfile::TempDir;
use tokio::time::timeout;
use tracing::error;
use tracing_test::traced_test;

use super::{
    GRAPHQL_TIMEOUT_DURATION, RunningRetailSupergraph, reserve_local_port,
    run_subgraphs_retail_supergraph, test_graphql_connection,
};

const ROVER_DEV_TIMEOUT: Duration = Duration::from_secs(45);
const ROVER_DEV_SUPERGRAPH_OUTPUT_FILE: &str = "composed-supergraph.graphql";
const ROVER_DEV_DYNAMIC_ROUTER_CONFIG_FILE: &str = "router-config-dev.dynamic.yaml";
const ROVER_DEV_LOG_LINE_CAP: usize = 200;

fn write_router_config_with_health_port(
    working_dir: &std::path::Path,
    health_port: u16,
) -> std::path::PathBuf {
    let base_config_path = working_dir.join("router-config-dev.yaml");
    let router_config_path = working_dir.join(ROVER_DEV_DYNAMIC_ROUTER_CONFIG_FILE);
    let base_router_config = std::fs::read_to_string(&base_config_path).unwrap_or_else(|err| {
        panic!(
            "Could not read router config at {}: {err}",
            base_config_path.display()
        )
    });
    std::fs::write(
        &router_config_path,
        format!("health_check:\n  listen: 127.0.0.1:{health_port}\n{base_router_config}"),
    )
    .unwrap_or_else(|err| {
        panic!(
            "Could not write router config at {}: {err}",
            router_config_path.display()
        )
    });
    router_config_path
}

fn spawn_rover_dev_log_reader<R: BufRead + Send + 'static>(
    reader: R,
    logs: Arc<Mutex<VecDeque<String>>>,
) {
    thread::spawn(move || {
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    eprintln!("{line}");
                    let mut logs = logs.lock().expect("rover dev log buffer poisoned");
                    if logs.len() >= ROVER_DEV_LOG_LINE_CAP {
                        logs.pop_front();
                    }
                    logs.push_back(line);
                }
                Err(err) => {
                    eprintln!("failed to read rover dev output: {err}");
                    break;
                }
            }
        }
    });
}

#[fixture]
#[once]
#[allow(clippy::zombie_processes)]
fn run_rover_dev(run_subgraphs_retail_supergraph: &RunningRetailSupergraph) -> String {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    let working_dir = run_subgraphs_retail_supergraph
        .retail_supergraph
        .working_dir
        .path();
    let (supergraph_listener, port) = reserve_local_port().expect("No ports free");
    let (health_listener, health_port) =
        reserve_local_port().expect("No ports free for health check");
    let router_config_path = write_router_config_with_health_port(working_dir, health_port);
    let router_config_arg = router_config_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("valid router config file name");
    let router_url = format!("http://localhost:{port}");
    let client = Client::new();

    cmd.args([
        "dev",
        "--supergraph-config",
        "supergraph-config-dev.yaml",
        "--router-config",
        router_config_arg,
        "--supergraph-port",
        &port.to_string(),
        "--supergraph-output",
        ROVER_DEV_SUPERGRAPH_OUTPUT_FILE,
        "--elv2-license",
        "accept",
    ]);
    cmd.current_dir(working_dir);
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", version);
    };
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", version);
    };
    if let Ok(version) = env::var("APOLLO_ROVER_DEV_MCP_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_MCP_VERSION", version);
    };
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    drop(supergraph_listener);
    drop(health_listener);
    let logs = Arc::new(Mutex::new(VecDeque::with_capacity(ROVER_DEV_LOG_LINE_CAP)));
    let mut child = cmd.spawn().expect("Could not run rover dev command");
    if let Some(stdout) = child.stdout.take() {
        spawn_rover_dev_log_reader(BufReader::new(stdout), Arc::clone(&logs));
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_rover_dev_log_reader(BufReader::new(stderr), Arc::clone(&logs));
    }
    tokio::task::block_in_place(|| {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(test_graphql_connection(
            &client,
            &router_url,
            ROVER_DEV_TIMEOUT,
            Some(&mut child),
        ))
    })
    .unwrap_or_else(|e| {
        let captured = logs
            .lock()
            .expect("rover dev log buffer poisoned")
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "rover dev failed to become reachable at {router_url}: {e}\n--- captured rover dev output ---\n{captured}"
        );
    });
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
#[serial]
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

#[ignore]
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
#[serial]
async fn e2e_test_rover_dev_writes_supergraph_output(
    #[from(run_rover_dev)] _router_url: &str,
    run_subgraphs_retail_supergraph: &RunningRetailSupergraph,
) {
    let output_path = run_subgraphs_retail_supergraph
        .retail_supergraph
        .working_dir
        .path()
        .join(ROVER_DEV_SUPERGRAPH_OUTPUT_FILE);
    let contents = std::fs::read_to_string(&output_path).unwrap_or_else(|err| {
        panic!("expected a composed supergraph at {output_path:?}, but couldn't read it: {err}")
    });
    // A composed Federation 2 supergraph always links the join/link specs.
    assert_that(&contents).contains("@link");
}

/// Test for issue #2751: Router config env var double expansion bug
///
/// When router.yaml contains `${env.VAR}` and VAR's value contains a `$`,
/// rover should NOT expand the env var - the router handles expansion itself.
#[ignore]
#[tokio::test]
#[traced_test]
#[serial]
async fn e2e_test_router_config_env_var_with_dollar_sign() {
    let temp_dir = TempDir::new().expect("Could not create temp directory");
    let temp_path = temp_dir.path();

    let schema = r#"
extend schema @link(url: "https://specs.apollo.dev/federation/v2.0", import: ["@key"])

type Query {
    hello: String
}
"#;
    std::fs::write(temp_path.join("schema.graphql"), schema)
        .expect("Could not write schema.graphql");

    // Supergraph config
    std::fs::write(
        temp_path.join("supergraph.yaml"),
        r#"
federation_version: =2.4.7
subgraphs:
  api:
    routing_url: http://localhost:4001
    schema:
      file: schema.graphql
"#,
    )
    .expect("Could not write supergraph.yaml");

    let (supergraph_listener, port) = reserve_local_port().expect("No ports free");
    let (health_listener, health_port) =
        reserve_local_port().expect("No ports free for health check");

    // Router config with env var reference - the key part of this test
    std::fs::write(
        temp_path.join("router.yaml"),
        format!(
            r#"
health_check:
  listen: 127.0.0.1:{health_port}
telemetry:
  exporters:
    tracing:
      common:
        service_name: ${{env.SERVICE_NAME}}
"#
        ),
    )
    .expect("Could not write router.yaml");

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "dev",
        "--supergraph-config",
        "supergraph.yaml",
        "--router-config",
        "router.yaml",
        "--supergraph-port",
        &port.to_string(),
        "--elv2-license",
        "accept",
    ]);
    cmd.current_dir(temp_path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.env("SERVICE_NAME", "my$service"); // $ in value triggers the bug
    if let Ok(v) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", v);
    }
    if let Ok(v) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", v);
    }
    if let Ok(v) = env::var("APOLLO_ROVER_DEV_MCP_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_MCP_VERSION", v);
    }

    drop(supergraph_listener);
    drop(health_listener);
    let mut child = cmd.spawn().expect("Failed to spawn rover dev");

    // Wait for the router to start and make a request
    let client = Client::new();
    let router_url = format!("http://localhost:{port}");
    let graphql_result =
        test_graphql_connection(&client, &router_url, ROVER_DEV_TIMEOUT, Some(&mut child)).await;

    // On Unix, send SIGINT so Rover can gracefully shut down the router (rover handles ctrl_c/SIGINT)
    // On Windows, use taskkill /T to kill the entire process tree since child.kill() only kills
    // rover, not the router subprocess, causing wait_with_output() to hang
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .output();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .output();
    }

    let output = child.wait_with_output().expect("Failed to get output");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // Assert no double expansion error messages
    assert_that(&combined).does_not_contain("could not expand variable");
    assert_that(&combined).does_not_contain("no valid configuration was supplied");

    // Assert router started successfully and responded to GraphQL introspection
    assert_that(&graphql_result).is_ok();
}

const CREDENTIAL_FREE_NOTICE: &str = "Running without GraphOS credentials. GraphOS Router Enterprise features and @connect are disabled. Pass --graph-ref, set APOLLO_KEY/APOLLO_GRAPH_REF, or pass --license to enable them.";

/// Spec FR8/FR9/FR10: `rover dev` prints a one-time startup notice when it has no usable
/// GraphOS credentials (no `--graph-ref`, no `APOLLO_KEY`/`APOLLO_GRAPH_REF`, no `--license`).
/// The test isolates the spawned process from whatever credentials happen to be configured on
/// the machine running the test (a real profile, ambient `APOLLO_KEY`/`APOLLO_GRAPH_REF`) so the
/// assertion holds regardless of the local/CI environment.
#[ignore]
#[tokio::test]
#[traced_test]
#[serial]
async fn e2e_test_prints_notice_when_no_credentials_are_configured() {
    let temp_dir = TempDir::new().expect("Could not create temp directory");
    let temp_path = temp_dir.path();
    let isolated_apollo_home = TempDir::new().expect("Could not create isolated APOLLO_HOME");

    let schema = r#"
type Query {
    hello: String
}
"#;
    std::fs::write(temp_path.join("schema.graphql"), schema)
        .expect("Could not write schema.graphql");

    let (supergraph_listener, port) = reserve_local_port().expect("No ports free");
    let (health_listener, health_port) =
        reserve_local_port().expect("No ports free for health check");

    std::fs::write(
        temp_path.join("supergraph.yaml"),
        r#"
federation_version: =2.4.7
subgraphs:
  api:
    routing_url: http://localhost:4001
    schema:
      file: schema.graphql
"#,
    )
    .expect("Could not write supergraph.yaml");

    std::fs::write(
        temp_path.join("router.yaml"),
        format!("health_check:\n  listen: 127.0.0.1:{health_port}\n"),
    )
    .expect("Could not write router.yaml");

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "dev",
        "--supergraph-config",
        "supergraph.yaml",
        "--router-config",
        "router.yaml",
        "--supergraph-port",
        &port.to_string(),
        "--elv2-license",
        "accept",
    ]);
    cmd.current_dir(temp_path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    // Isolate this process from whatever credentials are actually configured wherever this test
    // runs, so the "no credentials" assertion below is deterministic.
    cmd.env_remove("APOLLO_KEY");
    cmd.env_remove("APOLLO_GRAPH_REF");
    cmd.env("APOLLO_HOME", isolated_apollo_home.path());
    if let Ok(v) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", v);
    }
    if let Ok(v) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", v);
    }

    drop(supergraph_listener);
    drop(health_listener);
    let mut child = cmd.spawn().expect("Failed to spawn rover dev");

    let client = Client::new();
    let router_url = format!("http://localhost:{port}");
    let graphql_result =
        test_graphql_connection(&client, &router_url, ROVER_DEV_TIMEOUT, Some(&mut child)).await;

    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .output();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .output();
    }

    let output = child.wait_with_output().expect("Failed to get output");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_that(&graphql_result).is_ok();
    assert_eq!(
        combined.matches(CREDENTIAL_FREE_NOTICE).count(),
        1,
        "expected the credential-free notice exactly once, got:\n{combined}"
    );
}

/// Regression test: `--license <path>` used to be silently ignored (never read anywhere after a
/// prior `rover dev` rewrite), so passing a bogus path had no effect and the router started up
/// fine. Now that it's forwarded to the router's argv, a bogus license file makes the router
/// itself reject it and fail to start -- proving the flag actually reaches the router.
#[ignore]
#[tokio::test]
#[traced_test]
#[serial]
async fn e2e_test_bogus_license_file_is_forwarded_to_router() {
    let temp_dir = TempDir::new().expect("Could not create temp directory");
    let temp_path = temp_dir.path();
    let isolated_apollo_home = TempDir::new().expect("Could not create isolated APOLLO_HOME");

    let schema = r#"
type Query {
    hello: String
}
"#;
    std::fs::write(temp_path.join("schema.graphql"), schema)
        .expect("Could not write schema.graphql");

    let bogus_license_path = temp_path.join("license.jwt");
    std::fs::write(&bogus_license_path, "not-a-real-license")
        .expect("Could not write bogus license file");

    let (supergraph_listener, port) = reserve_local_port().expect("No ports free");
    let (health_listener, health_port) =
        reserve_local_port().expect("No ports free for health check");

    std::fs::write(
        temp_path.join("supergraph.yaml"),
        r#"
federation_version: =2.4.7
subgraphs:
  api:
    routing_url: http://localhost:4001
    schema:
      file: schema.graphql
"#,
    )
    .expect("Could not write supergraph.yaml");

    std::fs::write(
        temp_path.join("router.yaml"),
        format!("health_check:\n  listen: 127.0.0.1:{health_port}\n"),
    )
    .expect("Could not write router.yaml");

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "dev",
        "--supergraph-config",
        "supergraph.yaml",
        "--router-config",
        "router.yaml",
        "--supergraph-port",
        &port.to_string(),
        "--elv2-license",
        "accept",
        "--license",
        bogus_license_path.to_str().expect("valid utf8 path"),
    ]);
    cmd.current_dir(temp_path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.env_remove("APOLLO_KEY");
    cmd.env_remove("APOLLO_GRAPH_REF");
    cmd.env("APOLLO_HOME", isolated_apollo_home.path());
    if let Ok(v) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", v);
    }
    if let Ok(v) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", v);
    }

    drop(supergraph_listener);
    drop(health_listener);
    let mut child = cmd.spawn().expect("Failed to spawn rover dev");

    let client = Client::new();
    let router_url = format!("http://localhost:{port}");
    // The router should fail fast on the bogus license rather than ever becoming healthy.
    let graphql_result =
        test_graphql_connection(&client, &router_url, ROVER_DEV_TIMEOUT, Some(&mut child)).await;

    // The router should have already exited on its own after rejecting the bogus license, but
    // shut down defensively in case it (or `rover dev` itself) is still running.
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .output();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .output();
    }

    let output = child.wait_with_output().expect("Failed to get output");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_that(&graphql_result).is_err();
    assert_that(&combined).does_not_contain("Your supergraph is running!");
    // A valid --license present, even a rejected one, means the credential-free notice's
    // condition never held in the first place -- it must not print alongside the router's own
    // license-rejection error.
    assert_that(&combined).does_not_contain(CREDENTIAL_FREE_NOTICE);
}

/// Spec FR9: the credential-free notice must not fire when a different credential-related
/// warning already covers the same gap -- here, a non-default `--profile` whose credential
/// can't be resolved. Exactly one of the two messages should ever appear for a given session.
#[ignore]
#[tokio::test]
#[traced_test]
#[serial]
async fn e2e_test_unresolvable_profile_warning_suppresses_credential_free_notice() {
    let temp_dir = TempDir::new().expect("Could not create temp directory");
    let temp_path = temp_dir.path();
    // Left empty so the named profile below has nothing to resolve.
    let isolated_apollo_home = TempDir::new().expect("Could not create isolated APOLLO_HOME");

    let schema = r#"
type Query {
    hello: String
}
"#;
    std::fs::write(temp_path.join("schema.graphql"), schema)
        .expect("Could not write schema.graphql");

    let (supergraph_listener, port) = reserve_local_port().expect("No ports free");
    let (health_listener, health_port) =
        reserve_local_port().expect("No ports free for health check");

    std::fs::write(
        temp_path.join("supergraph.yaml"),
        r#"
federation_version: =2.4.7
subgraphs:
  api:
    routing_url: http://localhost:4001
    schema:
      file: schema.graphql
"#,
    )
    .expect("Could not write supergraph.yaml");

    std::fs::write(
        temp_path.join("router.yaml"),
        format!("health_check:\n  listen: 127.0.0.1:{health_port}\n"),
    )
    .expect("Could not write router.yaml");

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "dev",
        "--supergraph-config",
        "supergraph.yaml",
        "--router-config",
        "router.yaml",
        "--supergraph-port",
        &port.to_string(),
        "--elv2-license",
        "accept",
        "--profile",
        "e2e-test-nonexistent-profile",
    ]);
    cmd.current_dir(temp_path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.env_remove("APOLLO_KEY");
    cmd.env_remove("APOLLO_GRAPH_REF");
    cmd.env("APOLLO_HOME", isolated_apollo_home.path());
    if let Ok(v) = env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", v);
    }
    if let Ok(v) = env::var("APOLLO_ROVER_DEV_ROUTER_VERSION") {
        cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", v);
    }

    drop(supergraph_listener);
    drop(health_listener);
    let mut child = cmd.spawn().expect("Failed to spawn rover dev");

    let client = Client::new();
    let router_url = format!("http://localhost:{port}");
    let graphql_result =
        test_graphql_connection(&client, &router_url, ROVER_DEV_TIMEOUT, Some(&mut child)).await;

    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .output();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .output();
    }

    let output = child.wait_with_output().expect("Failed to get output");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_that(&graphql_result).is_ok();
    assert_that(&combined).contains("Could not retrieve APOLLO_KEY for profile");
    assert_that(&combined).does_not_contain(CREDENTIAL_FREE_NOTICE);
}
