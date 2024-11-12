use std::process::Command;
use std::{collections::HashMap, time::Duration};

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use reqwest::Client;

use rover_client::{
    blocking::GraphQLClient,
    operations::graph::introspect::{self, GraphIntrospectInput},
};
use rover_std::Fs;

const SCHEMA_PATH: &str = "./src/command/template/schema.graphql";
const QUERIES_PATH: &str = "./src/command/template/queries.graphql";

/// This script downloads the schema if it's not in the file system
/// or if we can detect the user is internet connected.
///
/// If the user is offline and the schema already exists in the file system, the script does nothing.
///
/// The URL to fetch the schema can be overridden with the APOLLO_GRAPHQL_SCHEMA_URL environment variable.
pub async fn update() -> Result<()> {
    let sdl = introspect().await?;

    let schema_path = Utf8PathBuf::from(SCHEMA_PATH);
    Fs::write_file(schema_path, sdl)?;
    regenerate_queries()
}

async fn introspect() -> Result<String> {
    let graphql_endpoint = "https://rover.apollo.dev/templates";
    crate::info!(
        "fetching the latest templates schema by introspecting {}...",
        &graphql_endpoint
    );
    let graphql_client = GraphQLClient::new(
        graphql_endpoint,
        Client::new(),
        Some(Duration::from_secs(10)),
    );
    introspect::run(
        GraphIntrospectInput {
            headers: HashMap::new(),
        },
        &graphql_client,
        false,
    )
    .await
    .map(|response| response.schema_sdl)
    .map_err(|err| err.into())
}

fn regenerate_queries() -> Result<()> {
    // Run this command:
    // graphql-client generate --schema-path schema.graphql queries.graphql \
    //   --response-derives 'Debug,Serialize,PartialEq,Eq,Clone' \
    //   --custom-scalars-module crate::command::template::custom_scalars
    // Return a suggestion to install graphql-client-cli if missing

    let output = Command::new("graphql-client")
        .arg("generate")
        .arg("--schema-path")
        .arg(SCHEMA_PATH)
        .arg(QUERIES_PATH)
        .arg("--response-derives")
        .arg("Debug,Serialize,PartialEq,Eq,Clone")
        .arg("--custom-scalars-module")
        .arg("crate::command::template::custom_scalars")
        .output();
    match output {
        Ok(output) => {
            if !output.status.success() {
                Err(anyhow!(
                    "failed to run graphql-client: {}",
                    String::from_utf8_lossy(&output.stderr)
                ))
            } else {
                Ok(())
            }
        }
        Err(err) => Err(anyhow!(
            "failed to run graphql-client: {}\n\
                Try installing it with `cargo install graphql_client_cli`",
            err
        )),
    }
}
