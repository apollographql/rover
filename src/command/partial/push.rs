use anyhow::{Context, Result};
use rover_client::query::partial::push::{self, PushPartialSchemaResponse};
use serde::Serialize;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

use crate::client::get_studio_client;
use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Push {
    /// Path to a .graphql SDL file
    #[structopt(name = "SCHEMA_PATH", parse(from_os_str))]
    #[serde(skip_serializing)]
    schema_path: PathBuf,

    /// Variant of the graph in Apollo Studio
    #[structopt(long, default_value = "current")]
    #[serde(skip_serializing)]
    variant: String,

    /// ID of the graph in Apollo Studio to push to
    #[structopt(long)]
    #[serde(skip_serializing)]
    graph_name: String,

    /// Name of the configuration profile (default: "default")
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// Name of the implementing service in the graph to update with a new schema
    #[structopt(long)]
    #[serde(skip_serializing)]
    service_name: String,
}

impl Push {
    pub fn run(&self) -> Result<RoverStdout> {
        let client =
            get_studio_client(&self.profile_name).context("Failed to get studio client")?;

        tracing::info!(
            "Let's push this schema, {}@{}, mx. {}!",
            &self.graph_name,
            &self.variant,
            &self.profile_name
        );

        let schema_document = get_schema_from_file_path(&self.schema_path)
            .context("Failed while loading from SDL file")?;

        let push_response = push::run(
            push::push_partial_schema_mutation::Variables {
                id: self.graph_name.clone(),
                graph_variant: self.variant.clone(),
                name: self.service_name.clone(),
                active_partial_schema: push::push_partial_schema_mutation::PartialSchemaInput {
                    sdl: Some(schema_document),
                    hash: None,
                },
                revision: "".to_string(),
                url: "".to_string(),
            },
            &client,
        )
        .context("Failed while pushing to Apollo Studio")?;

        handle_response(push_response, &self.service_name, &self.graph_name);
        Ok(RoverStdout::None)
    }
}

fn handle_response(response: PushPartialSchemaResponse, service_name: &str, graph: &str) {
    if response.service_was_created {
        tracing::info!(
            "A new service called '{}' for the '{}' graph was created",
            service_name,
            graph
        );
    } else {
        tracing::info!(
            "The '{}' service for the '{}' graph was updated",
            service_name,
            graph
        );
    }

    if response.did_update_gateway {
        tracing::info!("The gateway for the '{}' graph was updated with a new schema, composed from the updated '{}' service", graph, service_name);
    } else {
        tracing::info!(
            "The gateway for the '{}' graph was NOT updated with a new schema",
            graph
        );
    }

    if let Some(errors) = response.composition_errors {
        tracing::error!(
            "The following composition errors occurred: \n{}",
            errors.join("\n")
        );
    }
}

fn get_schema_from_file_path(path: &PathBuf) -> Result<String> {
    if Path::exists(path) {
        let contents = std::fs::read_to_string(path)?;
        Ok(contents)
    } else {
        Err(anyhow::anyhow!(
            "Invalid path. No file found at {}",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{get_schema_from_file_path, handle_response, PushPartialSchemaResponse};
    use assert_fs::TempDir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn get_schema_from_file_path_loads() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("schema.graphql");
        let mut temp_file = File::create(file_path.clone()).unwrap();
        write!(temp_file, "type Query {{ hello: String! }}").unwrap();

        let schema = get_schema_from_file_path(&file_path).unwrap();
        assert_eq!(schema, "type Query { hello: String! }".to_string());
    }

    #[test]
    fn get_schema_from_file_path_errs_on_bad_path() {
        let empty_path = std::path::PathBuf::new().join("wow.graphql");
        let schema = get_schema_from_file_path(&empty_path);
        assert_eq!(schema.is_err(), true);
    }

    // this test is a bit weird, since we can't test the output. We just verify it
    // doesn't error
    #[test]
    fn handle_response_doesnt_error_with_all_successes() {
        let response = PushPartialSchemaResponse {
            schema_hash: Some("123456".to_string()),
            did_update_gateway: true,
            service_was_created: true,
            composition_errors: None,
        };

        handle_response(response, "accounts", "my-graph");
    }

    #[test]
    fn handle_response_doesnt_error_with_all_failures() {
        let response = PushPartialSchemaResponse {
            schema_hash: None,
            did_update_gateway: false,
            service_was_created: false,
            composition_errors: Some(vec![
                "a bad thing happened".to_string(),
                "another bad thing".to_string(),
            ]),
        };

        handle_response(response, "accounts", "my-graph");
    }

    // TODO: test the actual output of the logs whenever we do design work
    // for the commands :)
}
