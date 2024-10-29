use std::process::Command;

use assert_cmd::prelude::CommandCargoExt;
use rstest::*;

use crate::e2e::remote_supergraph_graphref;

#[rstest]
#[case::retries_500s(3, 500)]
#[case::retries_429s(3, 429)]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_rover_client_timeout_option(
    #[case] timeout: usize,
    #[case] status_code: u16,
    remote_supergraph_graphref: String,
) {
    // GIVEN
    //   - a server returning a status code set dynamically via test case
    //   - the rover binary
    let server = httpmock::MockServer::start();
    let mocked_route = server.mock(|when, then| {
        when.any_request();
        then.status(status_code);
    });

    let fake_registry = format!("http://{}/graphql", server.address());

    // WHEN
    //   - a command supporting the --client-timeout option is invoked
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.env("APOLLO_REGISTRY_URL", fake_registry);
    cmd.args([
        "subgraph",
        "fetch",
        "--name",
        "perf-subgraph-00",
        "--client-timeout",
        &timeout.to_string(),
        &remote_supergraph_graphref,
    ]);
    cmd.output().expect("Could not run command");

    // THEN
    //  - the number of calls is greater than the timeout. This is because the timeout is a
    //  Duration for how long the client will live and we use that Duration as the _same_ Duration
    //  for how long we'll retry. There being at least as many attempted calls as the timeout means
    //  the crate we use, backoff, is doing its job in both retrying the call but also adding
    //  expoential backoff and jitter; this works out for smaller timeouts, but after a certain
    //  threshold of retries, enough backoff would be added to make the calls lower than the
    //  overall timeout (understood as a number that gets used as seconds in a Duration)
    let calls = mocked_route.hits();
    assert!(calls >= timeout);
}
