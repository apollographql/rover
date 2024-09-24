use std::{collections::BTreeMap, io::Write, str::FromStr, sync::Arc, time::Duration};

use anyhow::Result;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig, SupergraphConfig};
use camino::Utf8PathBuf;
use futures::{lock::Mutex, StreamExt};
use httpmock::{Mock, MockServer};
use rover::composition::{
    subgraph_watchers::SubgraphWatchers,
    watchers::{subtask::SubtaskHandleStream, watcher::subgraph::SubgraphChanged},
};
use rstest::{fixture, rstest};
use serde_json::json;
use speculoos::prelude::*;
use tempfile::NamedTempFile;
use tokio::sync::mpsc::unbounded_channel;
use tracing_test::traced_test;
use uuid::Uuid;

use crate::graphql::SUBGRAPH_INTROSPECTION_QUERY;

#[fixture]
#[once]
fn mock_server() -> httpmock::MockServer {
    MockServer::start()
}

#[fixture]
fn sdl() -> String {
    format!(
        "type Query {{ test_{}: String! }}",
        Uuid::new_v4().as_simple()
    )
}

#[fixture]
fn subgraph_name() -> String {
    Uuid::new_v4().as_simple().to_string()
}

#[fixture]
fn introspection_subgraph<'a>(
    mock_server: &'a MockServer,
    sdl: String,
    subgraph_name: String,
) -> (String, SubgraphConfig, Mock<'a>) {
    let mock_server_address = mock_server.address();
    let root = format!(
        "http://{}:{}",
        mock_server_address.ip(),
        mock_server_address.port()
    );
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

#[fixture]
fn file_subgraph(sdl: String, subgraph_name: String) -> (String, SubgraphConfig, NamedTempFile) {
    let mut file = NamedTempFile::new().expect("Could not create temp file");
    file.write_all(sdl.as_bytes())
        .expect("Could not write SDL to temp file");
    let file_path = Utf8PathBuf::from_path_buf(file.path().to_path_buf())
        .expect("Could not convert file path to Utf8PathBuf");
    let subgraph_config = SubgraphConfig {
        routing_url: Some("http://example.com".to_string()),
        schema: SchemaSource::File { file: file_path },
    };
    (subgraph_name, subgraph_config, file)
}

#[rstest]
#[timeout(std::time::Duration::from_secs(5))]
#[traced_test]
#[tokio::test(flavor = "multi_thread")]
async fn test_setup_subgraph_watchers(
    mock_server: &'_ MockServer,
    introspection_subgraph: (String, SubgraphConfig, Mock<'_>),
    file_subgraph: (String, SubgraphConfig, NamedTempFile),
) -> Result<()> {
    let (
        introspection_subgraph_name,
        introspection_subgraph_config,
        mut introspection_subgraph_mock,
    ) = introspection_subgraph;
    let (file_subgraph_name, file_subgraph_config, _file) = file_subgraph;
    let subgraphs = BTreeMap::from_iter([
        (
            introspection_subgraph_name.to_string(),
            introspection_subgraph_config,
        ),
        (file_subgraph_name.to_string(), file_subgraph_config),
    ]);
    let supergraph_config = SupergraphConfig::new(subgraphs, None);
    let subgraph_watchers = SubgraphWatchers::new(supergraph_config);
    let (tx, rx) = unbounded_channel();
    let rx = Arc::new(Mutex::new(rx));
    let input_stream = futures::stream::empty().boxed();
    let _ = subgraph_watchers.handle(tx, input_stream);

    let timeout = tokio::time::timeout(Duration::from_millis(500), {
        let rx = rx.clone();
        async move {
            let mut output = None;
            if let Some(change) = rx.lock().await.recv().await {
                output = Some(change);
            }
            output
        }
    })
    .await;
    assert_that!(timeout)
        .is_ok()
        .is_some()
        .is_equal_to(SubgraphChanged::from(file_subgraph_name.to_string()));

    let timeout = tokio::time::timeout(Duration::from_millis(500), {
        let rx = rx.clone();
        async move {
            let mut output = None;
            if let Some(change) = rx.lock().await.recv().await {
                output = Some(change);
            }
            output
        }
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

    let timeout = tokio::time::timeout(Duration::from_millis(1500), {
        let rx = rx.clone();
        async move {
            let mut output = None;
            if let Some(change) = rx.lock().await.recv().await {
                output = Some(change);
            }
            output
        }
    })
    .await;

    assert_that!(timeout)
        .is_ok()
        .is_some()
        .is_equal_to(SubgraphChanged::from(introspection_subgraph_name));

    Ok(())
}
