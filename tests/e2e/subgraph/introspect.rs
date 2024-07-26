use std::fs::{read_to_string, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::process::Command;
use std::time::Duration;

use assert_cmd::prelude::CommandCargoExt;
use rstest::rstest;
use serde_json::{json, Value};
use speculoos::assert_that;
use tempfile::{Builder, TempDir};

use crate::e2e::{
    get_supergraph_config, run_single_mutable_subgraph, run_subgraphs_retail_supergraph,
    RETAIL_SUPERGRAPH_SCHEMA_NAME,
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

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_rover_subgraph_introspect_watch(
    #[from(run_single_mutable_subgraph)]
    #[future]
    subgraph_details: (String, TempDir, String),
) {
    // Set up the command to output the original file
    let mut out_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Could not create output file");
    let (url, subgraph_dir, schema_name) = subgraph_details.await;
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args([
        "subgraph",
        "introspect",
        &url,
        "--watch",
        "--format",
        "json",
        "--output",
        out_file.path().to_str().unwrap(),
    ]);
    let mut child = cmd.spawn().expect("Could not run command");
    tokio::time::sleep(Duration::from_secs(1)).await;
    // Store the result
    let original_value: Value = serde_json::from_reader(out_file.as_file()).unwrap();
    // Make a change to the schema
    let schema_path = subgraph_dir.path().join(schema_name);
    let schema = read_to_string(&schema_path).expect("Could not read schema file");
    let new_schema = schema.replace("allPandas", "getMeAllThePandas");
    let mut schema_file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(schema_path)
        .expect("Cannot open schema file");
    schema_file
        .write(new_schema.as_bytes())
        .expect("Could not update schema");
    tokio::time::sleep(Duration::from_secs(2)).await;
    // Get the new result
    out_file
        .seek(SeekFrom::Start(0))
        .expect("Could not rewind file");
    let new_value: Value = serde_json::from_reader(out_file.as_file()).unwrap();
    // Ensure that the two are different
    assert_that!(new_value).is_not_equal_to(original_value);
    // Ensure the changed schema is what we expect it to be
    let expected_value = json!({
       "data":{
          "introspection_response":"directive @tag(name: String!) repeatable on FIELD_DEFINITION\n\ndirective @key(fields: _FieldSet!, resolvable: Boolean = true) repeatable on OBJECT | INTERFACE\n\ndirective @requires(fields: _FieldSet!) on FIELD_DEFINITION\n\ndirective @provides(fields: _FieldSet!) on FIELD_DEFINITION\n\ndirective @external(reason: String) on OBJECT | FIELD_DEFINITION\n\ndirective @extends on OBJECT | INTERFACE\n\ntype Query {\n  getMeAllThePandas: [Panda]\n  panda(name: ID!): Panda\n  _service: _Service!\n}\n\ntype Panda {\n  name: ID!\n  favoriteFood: String @tag(name: \"nom-nom-nom\")\n}\n\nscalar _FieldSet\n\nscalar _Any\n\ntype _Service {\n  sdl: String\n}",
          "success":true
       },
       "error":null,
       "json_version":"1"
    });
    assert_that!(new_value).is_equal_to(expected_value);
    child.kill().unwrap();
}
