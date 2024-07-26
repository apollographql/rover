use std::process::Command;

use assert_cmd::prelude::CommandCargoExt;
use rstest::rstest;
use serde_json::{json, Value};
use speculoos::assert_that;
use tempfile::{Builder, TempDir};

use crate::e2e::{
    get_supergraph_config, run_subgraphs_retail_supergraph, RETAIL_SUPERGRAPH_SCHEMA_NAME,
};

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_rover_subgraph_introspect(
    #[from(run_subgraphs_retail_supergraph)] supergraph_dir: &TempDir,
) {
    // Extract the inventory URL from the supergraph.yaml
    let supergraph_config_path = supergraph_dir.path().join(RETAIL_SUPERGRAPH_SCHEMA_NAME);
    let url = get_supergraph_config(supergraph_config_path)
        .subgraphs
        .get("inventory")
        .unwrap()
        .routing_url
        .clone();

    // Set up the command to output
    let out_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Could not create output file");
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args([
        "subgraph",
        "introspect",
        &url,
        "--format",
        "json",
        "--output",
        out_file.path().to_str().unwrap(),
    ]);
    cmd.output().expect("Could not run command");

    // Slurp the output and then compare it to the canonical one
    let actual_value: Value = serde_json::from_reader(out_file.as_file()).unwrap();
    let expected_value = json!({
        "data":{
          "introspection_response":"extend schema\n  @link(url: \"https://specs.apollo.dev/link/v1.0\")\n  @link(url: \"https://specs.apollo.dev/federation/v2.0\", import: [\"@key\"])\n\ndirective @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA\n\ndirective @key(fields: federation__FieldSet!, resolvable: Boolean = true) repeatable on OBJECT | INTERFACE\n\ndirective @federation__requires(fields: federation__FieldSet!) on FIELD_DEFINITION\n\ndirective @federation__provides(fields: federation__FieldSet!) on FIELD_DEFINITION\n\ndirective @federation__external(reason: String) on OBJECT | FIELD_DEFINITION\n\ndirective @federation__tag(name: String!) repeatable on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION\n\ndirective @federation__extends on OBJECT | INTERFACE\n\ndirective @federation__shareable on OBJECT | FIELD_DEFINITION\n\ndirective @federation__inaccessible on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION\n\ndirective @federation__override(from: String!) on FIELD_DEFINITION\n\ntype Variant\n  @key(fields: \"id\")\n{\n  id: ID!\n\n  \"\"\"Checks the warehouse API for inventory information.\"\"\"\n  inventory: Inventory\n}\n\n\"\"\"Inventory details about a specific Variant\"\"\"\ntype Inventory {\n  \"\"\"Returns true if the inventory count is greater than 0\"\"\"\n  inStock: Boolean!\n\n  \"\"\"The raw count of not purchased items in the warehouse\"\"\"\n  inventory: Int\n}\n\nenum link__Purpose {\n  \"\"\"\n  `SECURITY` features provide metadata necessary to securely resolve fields.\n  \"\"\"\n  SECURITY\n\n  \"\"\"\n  `EXECUTION` features provide metadata necessary for operation execution.\n  \"\"\"\n  EXECUTION\n}\n\nscalar link__Import\n\nscalar federation__FieldSet\n\nscalar _Any\n\ntype _Service {\n  sdl: String\n}\n\nunion _Entity = Variant\n\ntype Query {\n  _entities(representations: [_Any!]!): [_Entity]!\n  _service: _Service!\n}",
          "success":true
        },
        "error":null,
        "json_version":"1"
    });

    assert_that!(actual_value).is_equal_to(expected_value);
}
