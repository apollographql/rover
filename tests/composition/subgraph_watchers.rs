use std::{collections::BTreeMap, str::FromStr, sync::Arc, time::Duration};

use anyhow::Result;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig, SupergraphConfig};
use futures::{lock::Mutex, StreamExt};
use httpmock::{Mock, MockServer};
use rover::composition::{
    subgraph_watchers::SubgraphWatchers,
    watchers::{subtask::SubtaskHandleStream, watcher::subgraph::SubgraphChanged},
};
use rstest::{fixture, rstest};
use serde_json::json;
use speculoos::prelude::*;
use tokio::sync::mpsc::unbounded_channel;
use uuid::Uuid;

use crate::graphql::SUBGRAPH_INTROSPECTION_QUERY;

#[fixture]
#[once]
pub fn mock_server() -> httpmock::MockServer {
    MockServer::start()
}

#[fixture]
pub fn sdl() -> String {
    format!(
        "type Query {{ test_{}: String! }}",
        Uuid::new_v4().as_simple()
    )
}

#[fixture]
pub fn introspection_subgraph<'a>(
    mock_server: &'a MockServer,
    sdl: String,
) -> (String, SubgraphConfig, Mock<'a>) {
    let mock_server_address = mock_server.address();
    let root = format!(
        "http://{}:{}",
        mock_server_address.ip(),
        mock_server_address.port()
    );
    let subgraph_name = Uuid::new_v4().as_simple().to_string();
    let routing_url = format!("{}/{}", root, subgraph_name);
    let mock = mock_server.mock(|when, then| {
        let expected_body = json!({
            "query": SUBGRAPH_INTROSPECTION_QUERY,
            "variables": null,
            "operationName": "SubgraphIntrospectQuery"
        });
        when.path(format!("/{}", subgraph_name))
            .method(httpmock::Method::POST)
            .json_body_obj(&expected_body);
        then.status(200).json_body(json!({
            "data": {
                "_service": {
                    "sdl": sdl
                }
            }
        }));
    });
    let subgraph_config = SubgraphConfig {
        routing_url: Some(routing_url.clone()),
        schema: SchemaSource::SubgraphIntrospection {
            subgraph_url: url::Url::from_str(&routing_url).expect("Invalid Url"),
            introspection_headers: None,
        },
    };
    (subgraph_name, subgraph_config, mock)
}

#[rstest]
#[tokio::test]
pub async fn test_setup_subgraph_watchers(
    mock_server: &'_ MockServer,
    introspection_subgraph: (String, SubgraphConfig, Mock<'_>),
) -> Result<()> {
    let (
        introspection_subgraph_name,
        introspection_subgraph_config,
        mut introspection_subgraph_mock,
    ) = introspection_subgraph;
    let subgraphs = BTreeMap::from_iter([(
        introspection_subgraph_name.to_string(),
        introspection_subgraph_config,
    )]);
    let supergraph_config = SupergraphConfig::new(subgraphs, None);
    let subgraph_watchers = SubgraphWatchers::new(supergraph_config);
    let (tx, rx) = unbounded_channel();
    let rx_1 = Arc::new(Mutex::new(rx));
    let rx_2 = rx_1.clone();
    let input_stream = futures::stream::empty().boxed();
    let _abort_handle = subgraph_watchers.handle(tx, input_stream);
    let timeout = tokio::time::timeout(Duration::from_millis(500), async move {
        let mut output = None;
        if let Some(change) = rx_1.lock().await.recv().await {
            output = Some(change);
        }
        output
    })
    .await;
    assert_that!(timeout)
        .is_ok()
        .is_some()
        .is_equal_to(SubgraphChanged::from(
            introspection_subgraph_name.to_string(),
        ));

    introspection_subgraph_mock.delete();

    mock_server.mock(|when, then| {
        let expected_body = json!({
            "query": SUBGRAPH_INTROSPECTION_QUERY,
            "variables": null,
            "operationName": "SubgraphIntrospectQuery"
        });
        when.path(format!("/{}", introspection_subgraph_name))
            .method(httpmock::Method::POST)
            .json_body_obj(&expected_body);
        then.status(200).json_body(json!({
            "data": {
                "_service": {
                    "sdl": sdl()
                }
            }
        }));
    });

    let timeout = tokio::time::timeout(Duration::from_millis(1500), async move {
        let mut output = None;
        if let Some(change) = rx_2.lock().await.recv().await {
            output = Some(change);
        }
        output
    })
    .await;
    assert_that!(timeout)
        .is_ok()
        .is_some()
        .is_equal_to(SubgraphChanged::from(introspection_subgraph_name));
    Ok(())
}
