use std::fs;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use reqwest::blocking::{Client, RequestBuilder};
use rover_std::Fs;
use uuid::Uuid;

const SCHEMA_DIR: &str = "./crates/rover-client/.schema";

/// This script downloads the schema if it's not in the file system
/// or if we can detect the user is internet connected.
///
/// If the user is offline and the schema already exists in the file system, the script does nothing.
///
/// The URL to fetch the schema can be overridden with the APOLLO_GRAPHQL_SCHEMA_URL environment variable.
pub fn update() -> Result<()> {
    let schema_dir = Utf8PathBuf::from(SCHEMA_DIR);
    Fs::create_dir_all(&schema_dir)?;
    let last_run_uuid = Uuid::new_v4().to_string();
    Fs::write_file(schema_dir.join("last_run.uuid"), last_run_uuid)?;

    let hash_path = schema_dir.join("hash.id");

    // skip updating the schema if we already have an etag or we're offline
    let should_update_schema = !(hash_path.exists()) || online::check(None).is_ok();

    if should_update_schema {
        if !(hash_path.exists()) {
            crate::info!("{} doesn't exist", &hash_path);
        } else {
            crate::info!("{} already exists", &hash_path);
            let current_hash = Fs::read_file(hash_path)?;
            crate::info!("current hash: {}", current_hash);
            let remote_hash = query_hash()?;

            if remote_hash == current_hash {
                crate::info!("hashes match. not updating schema.");
                return Ok(());
            }
        }
        let (remote_hash, remote_schema) = query_schema_and_hash()?;
        update_schema(&remote_hash, &remote_schema)
    } else {
        Ok(())
    }
}

fn query_hash() -> Result<String> {
    let (hash, _) = query(false)?;
    Ok(hash)
}

fn query_schema_and_hash() -> Result<(String, String)> {
    let (hash, schema) = query(true)?;
    Ok((hash, schema.unwrap()))
}

fn update_schema(hash: &str, schema: &str) -> Result<()> {
    let schema_dir = Utf8PathBuf::from(SCHEMA_DIR);

    let hash_path = schema_dir.join("hash.id");

    Fs::write_file(hash_path, hash)?;
    Fs::write_file(schema_dir.join("schema.graphql"), schema)?;

    // old versions of Rover wrote to etag.id, since this is no longer needed,
    // let's remove it from dev machines
    let _ = fs::remove_file(schema_dir.join("etag.id"));

    Ok(())
}

const QUERY: &str = r#"query FetchSchema($fetchDocument: Boolean!, $graphId: ID!, $variant: String!) {
  graph(id: $graphId) {
    variant(name: $variant) {
      latestPublication {
        schema {
          hash
          document @include(if: $fetchDocument)
        }
      }
    }
  }
}"#;

enum GraphOsStack {
    Production,
    Staging,
}

impl GraphOsStack {
    fn from_env() -> Self {
        match option_env!("APOLLO_GRAPHOS_STACK") {
            Some("production") | Some("prod") | None => Self::Production,
            Some("staging") => Self::Staging,
            _ => panic!(
                "invalid value for $APOLLO_GRAPHOS_STACK, expected 'production' or 'staging'"
            ),
        }
    }

    fn endpoint(&self) -> &str {
        match &self {
            Self::Production => "https://api.apollographql.com/api/graphql",
            Self::Staging => "https://graphql-staging.api.apollographql.com/api/graphql",
        }
    }

    fn graph_id(&self) -> &str {
        match &self {
            Self::Production => "apollo-platform",
            Self::Staging => "apollo-platform",
        }
    }

    fn variant(&self) -> &str {
        match &self {
            GraphOsStack::Production => "main",
            GraphOsStack::Staging => "staging",
        }
    }

    fn add_headers(&self, request_builder: RequestBuilder) -> RequestBuilder {
        if let Self::Staging = &self {
            let key =
                std::env::var("APOLLO_KEY").expect("need $APOLLO_KEY set to fetch staging schema");
            request_builder
                .header("x-api-key", key)
                .header("apollo-sudo", "true")
        } else {
            request_builder
        }
    }
}

fn query(fetch_document: bool) -> Result<(String, Option<String>)> {
    let graphql_stack = GraphOsStack::from_env();

    let graphql_endpoint = graphql_stack.endpoint();
    if fetch_document {
        crate::info!(
            "fetching the latest schema via {}: graph.variant.latestPublication.schema.document...",
            &graphql_endpoint
        );
    } else {
        crate::info!(
            "fetching the latest hash via {}: graph.variant.latestPublication.schema.hash...",
            &graphql_endpoint
        );
    }
    let client = Client::new();
    let schema_query = serde_json::json!({
        "variables": {"fetchDocument": fetch_document, "graphId": graphql_stack.graph_id(), "variant": graphql_stack.variant()},
        "query": QUERY
    });
    let request_builder = client
        .post(graphql_endpoint)
        .json(&schema_query)
        .header("apollographql-client-name", "rover-client")
        .header(
            "apollographql-client-version",
            format!("{} (dev)", env!("CARGO_PKG_VERSION")),
        );

    let request_builder = graphql_stack.add_headers(request_builder);

    let response = request_builder.send()?.error_for_status()?;
    let json: serde_json::Value = response.json()?;
    if let Some(errors) = json.get("errors") {
        return Err(anyhow!("{:?}", errors));
    }
    let result = &json["data"]["graph"]["variant"]["latestPublication"]["schema"];
    let hash = result["hash"].as_str().unwrap().to_string();
    if !fetch_document {
        crate::info!(" latest hash: {}", &hash);
    }
    let maybe_document = result["document"].as_str().map(|s| s.to_string());
    Ok((hash, maybe_document))
}
