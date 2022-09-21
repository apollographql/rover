//! Tests for the `Thing` type, queries are in `thing.graphql`

use serde_json::{json, Value};

mod helpers;

async fn run_graphql_query(operation: &str) -> Value {
    helpers::run_graphql_query(include_str!("thing.graphql"), operation).await
}

#[tokio::test]
async fn get_thing() {
    let value = run_graphql_query("getThing").await;

    assert_eq!(value, json!({ "data": { "thing": { "name": "Name" } } }));
}

#[tokio::test]
async fn get_thing_entity() {
    let value = run_graphql_query("getThingEntity").await;

    assert_eq!(
        value,
        json!({ "data": { "_entities": [{"name": "Name" }] } })
    );
}

#[tokio::test]
async fn create_thing() {
    let value = run_graphql_query("createThing").await;

    assert_eq!(
        value,
        json!({ "data": { "createThing": { "name": null } } })
    );
}
