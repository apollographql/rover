use std::fs::{self, read, write};

use anyhow::Result;
use camino::Utf8PathBuf as PathBuf;
use reqwest::blocking::Client;
use uuid::Uuid;

/// This script downloads the schema if it's not in the file system
/// or if we can detect the user is internet connected.
///
/// If the user is offline and the schema already exists in the file system, the script does nothing.
///
/// The URL to fetch the schema can be overridden with the APOLLO_GPAPHQL_SCHEMA_URL environment variable.
///
/// Note: eprintln! statements only show up with `cargo build -vv`
fn main() -> Result<()> {
    // Rerun the build if this script updates last_run.uuid (which it does every time).
    eprintln!("cargo:rerun-if-changed=.schema/last_run.uuid");
    fs::create_dir_all(".schema")?;
    write(".schema/last_run.uuid", Uuid::new_v4())
        .expect("Failed to write UUID to .schema/last_run.uuid");

    let hash_path = PathBuf::from(".schema/hash.id");

    // skip updating the schema if we already have an etag or we're offline
    let should_update_schema = !(hash_path.exists()) || online::sync::check(None).is_ok();

    if should_update_schema {
        if !(hash_path.exists()) {
            eprintln!(".schema/hash.id doesn't exist");
        } else {
            eprintln!(".schema/hash.id already exists");
            let current_hash = String::from_utf8(read(hash_path)?).unwrap();
            eprintln!("current hash: {}", current_hash);
            let remote_hash = query_hash()?;

            if remote_hash == current_hash {
                eprintln!("hashes match. Not updating schema.");
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
    eprintln!("Updating schema.");

    eprintln!("Saving {} to .schema/hash.id", hash);
    write(".schema/hash.id", hash)?;

    eprintln!("Writing schema text to .schema/schema.graphql");

    write(".schema/schema.graphql", schema)?;

    // old versions of Rover wrote to etag.id, since this is no longer needed,
    // let's remove it from dev machines
    let _ = fs::remove_file("./.schema/etag.id");

    Ok(())
}

const QUERY: &str = r#"query FetchSchema($fetchDocument: Boolean!) {
  graph(id: "apollo-platform") {
    variant(name: "current") {
      latestPublication {
        schema {
          hash
          document @include(if: $fetchDocument)
        }
      }
    }
  }
}"#;

fn query(fetch_document: bool) -> Result<(String, Option<String>)> {
    let graphql_endpoint = option_env!("APOLLO_GRAPHQL_SCHEMA_URL")
        .unwrap_or_else(|| "https://api.apollographql.com/api/graphql");
    let client = Client::new();
    let schema_query = serde_json::json!({
        "variables": {"fetchDocument": fetch_document},
        "query": QUERY
    });
    let response = client
        .post(graphql_endpoint)
        .json(&schema_query)
        .send()?
        .error_for_status()?;
    let json: serde_json::Value = response.json()?;
    if let Some(errors) = json.get("errors") {
        return Err(anyhow::anyhow!("{:?}", errors));
    }
    let result = &json["data"]["graph"]["variant"]["latestPublication"]["schema"];
    let hash = result["hash"].as_str().unwrap().to_string();
    let maybe_document = result["document"].as_str().map(|s| s.to_string());
    Ok((hash, maybe_document))
}
