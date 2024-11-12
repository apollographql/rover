use std::fs;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use reqwest::Client;
use rover_std::Fs;
use uuid::Uuid;

const SCHEMA_DIR: &str = "./crates/rover-client/.schema";

/// Downloads the schema if it's not in the file system
/// or if we can detect the user is internet connected.
///
/// If the schema already exists in the file system and matches the remote hash, do nothing.
///
/// The URL to fetch the schema can be overridden with the APOLLO_GRAPHQL_SCHEMA_URL environment variable.
pub async fn update() -> Result<()> {
    let schema_dir = Utf8PathBuf::from(SCHEMA_DIR);
    Fs::create_dir_all(&schema_dir)?;
    let last_run_uuid = Uuid::new_v4().to_string();
    Fs::write_file(schema_dir.join("last_run.uuid"), last_run_uuid)?;

    let hash_path = schema_dir.join("hash.id");

    if !hash_path.exists() {
        crate::info!("{} doesn't exist", &hash_path);
    } else {
        crate::info!("{} already exists", &hash_path);
        let current_hash = Fs::read_file(hash_path)?;
        crate::info!("current hash: {}", current_hash);
        let remote_hash = query_hash().await?;

        if remote_hash == current_hash {
            crate::info!("hashes match. not updating schema.");
            return Ok(());
        }
    }
    let (remote_hash, remote_schema) = query_schema_and_hash().await?;
    update_schema(&remote_hash, &remote_schema)
}

async fn query_hash() -> Result<String> {
    let (hash, _) = query(false).await?;
    Ok(hash)
}

async fn query_schema_and_hash() -> Result<(String, String)> {
    let (hash, schema) = query(true).await?;
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

const QUERY: &str = r#"query FetchSchema($fetchDocument: Boolean!) {
  graph(id: "apollo-platform") {
    variant(name: "main") {
      latestPublication {
        schema {
          hash
          document @include(if: $fetchDocument)
        }
      }
    }
  }
}"#;

async fn query(fetch_document: bool) -> Result<(String, Option<String>)> {
    let graphql_endpoint = option_env!("APOLLO_GRAPHQL_SCHEMA_URL")
        .unwrap_or_else(|| "https://api.apollographql.com/api/graphql");
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
        "variables": {"fetchDocument": fetch_document},
        "query": QUERY
    });
    let response = client
        .post(graphql_endpoint)
        .json(&schema_query)
        .header("apollographql-client-name", "rover-client")
        .header(
            "apollographql-client-version",
            format!("{} (dev)", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await?
        .error_for_status()?;
    let json: serde_json::Value = response.json().await?;
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
