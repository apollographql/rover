use anyhow::Result;
use assert_cmd::Command;
use httpmock::{Method, MockServer};
use indoc::indoc;
use serde_json::json;

#[test]
fn it_produces_a_supergraph_yaml_config() -> Result<()> {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::POST);
        let response = json!({
            "data": {
                "frontendUrlRoot": "https://studio.apollographql.com/",
                "graph": {
                    "variant": {
                        "subgraphs": [
                            {
                                "name": "actors",
                                "url": "https://example.com/graphql",
                                "updatedAt": "2024-05-27T02:20:21.261Z"
                            }
                        ]
                    }
                }
            }
        });
        then.status(200)
            .header("content-type", "application/json")
            .json_body(response);
    });
    let expected_yaml = indoc! {r#"
      federation_version: '2'
      subgraphs:
        actors:
          routing_url: https://example.com/graphql
          schema:
            graphref: mygraph@current
            subgraph: actors

      "#
    };
    let mut cmd = Command::cargo_bin("rover")?;
    let result = cmd
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .arg("supergraph")
        .arg("config")
        .arg("fetch")
        .arg("mygraph@current")
        .assert();
    result.stdout(expected_yaml);

    Ok(())
}
