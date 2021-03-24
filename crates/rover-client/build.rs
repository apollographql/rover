use std::env;
use std::fs::{self, read, write};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use camino::Utf8PathBuf as PathBuf;
use reqwest::blocking::{Client, Request, Response};
use reqwest::{Method, Url};

use uuid::Uuid;

/// This script downloads the schema if it's not in the file system
/// or if we can detect the user is internet connected.
///
/// If the user is offline and the schema already exists in the file system, the script does nothing.
///
/// The URL to fetch the schema can be overriden with the APOLLO_GPAPHQL_SCHEMA_URL environment variable.
///
/// Note: eprintln! statements only show up with `cargo build -vv`
fn main() -> anyhow::Result<()> {
    // Rerun the build if this script updates last_run.uuid (which it does every time).
    eprintln!("cargo:rerun-if-changed=.schema/last_run.uuid");
    fs::create_dir_all(".schema")?;
    write(".schema/last_run.uuid", Uuid::new_v4().to_string())
        .expect("Failed to write UUID to .schema/last_run.uuid");

    let schema_url = env::var("APOLLO_GPAPHQL_SCHEMA_URL")
        .unwrap_or_else(|_| "https://graphql.api.apollographql.com/api/schema".to_owned());

    let client = Client::new();
    let etag_path = PathBuf::from(".schema/etag.id");

    let is_online = if let Ok(online) = online::online(None) {
        online
    } else {
        false
    };

    let should_update_schema = !(etag_path.exists()) || is_online;

    if should_update_schema {
        if !(etag_path.exists()) {
            eprintln!(".schema/etag.id doesn't exist");
            update_schema(&client, &schema_url)
        } else {
            eprintln!(".schema/etag.id already exists");
            let current_etag = String::from_utf8(read(etag_path)?).unwrap();
            eprintln!("current etag: {}", current_etag);

            let response = retry_request(&client, Method::HEAD, &schema_url)?;

            let remote_etag = response.headers().get("etag").and_then(|v| v.to_str().ok());
            eprintln!("remote etag: {}", remote_etag.unwrap_or("None"));

            if let Some(remote_etag) = remote_etag {
                if remote_etag == current_etag {
                    eprintln!("etags match. Not updating schema.");
                    return Ok(());
                }
            }

            update_schema(&client, &schema_url)
        }
    } else {
        Ok(())
    }
}

fn retry_request(client: &Client, method: Method, url: &str) -> Result<Response> {
    let mut delay_secs = 2;
    let max_delay_secs = 30;
    let mut has_failed = false;

    loop {
        let parsed_url = Url::parse(url)?;
        let request = Request::new(method.clone(), parsed_url);
        if let Ok(response) = client.execute(request) {
            if has_failed {
                eprintln!("Success!");
            }
            return Ok(response);
        } else {
            has_failed = true;
            eprintln!("failed to fetch {}.", url);
            eprintln!("retrying connection in {} seconds", delay_secs);
            thread::sleep(Duration::from_secs(delay_secs));
            delay_secs = delay_secs.pow(2);

            if delay_secs > max_delay_secs {
                delay_secs = max_delay_secs;
            }

            eprintln!("retrying fetch...");
        }
    }
}

fn update_schema(client: &Client, url: &str) -> anyhow::Result<()> {
    eprintln!("Updating schema.");
    let response = retry_request(client, Method::GET, url)?;

    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .expect("Failed to get etag header from response.");

    eprintln!("Saving {} to .schema/etag.id", etag);
    write(".schema/etag.id", etag)?;

    let schema = response
        .text()
        .expect("Failed to get schema text from response.");

    eprintln!("Writing schema text to .schema/schema.graphql");

    write(".schema/schema.graphql", schema)?;

    Ok(())
}
