#[test]
fn introspection_cli_tests() {
    trycmd::TestCases::new().case("e2e/introspection/*.md");
}

#[test]
fn connectors_cli_tests() {
    // These run `rover connector`, which downloads the `supergraph` plugin
    // on the fly, so they opt in to automatic downloads.
    trycmd::TestCases::new()
        .env("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "true")
        .case("e2e/connectors/*.md");
}
