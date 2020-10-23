use crate::client::get_rover_client;
use anyhow::Result;
use rover_client::query::service::push::{self, PushPartialSchemaResponse};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Push {
    /// where to find the schema. .graphql, .json or uri
    #[structopt(name = "SCHEMA_PATH", parse(from_os_str))]
    schema_path: PathBuf,

    /// The variant of the request graph from Apollo Studio
    #[structopt(long, default_value = "current")]
    variant: String,

    /// The unique graph name that this schema is being pushed to
    #[structopt(long)]
    graph_name: String,

    /// Name of the configuration profile (default: "default")
    #[structopt(long = "profile", default_value = "default")]
    profile_name: String,

    /// Name of the implementing service in the graph to update with a new schema
    #[structopt(long)]
    service_name: String,
}

impl Push {
    pub fn run(&self) -> Result<()> {
        let client = get_rover_client(&self.profile_name)?;

        log::info!(
            "Let's push this schema, {}@{}, mx. {}!",
            &self.graph_name,
            &self.variant,
            &self.profile_name
        );

        let schema_document = get_schema_from_file_path(&self.schema_path)?;

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
            client,
        )?;

        handle_response(push_response, &self.service_name, &self.graph_name);
        Ok(())
    }
}

fn handle_response(response: PushPartialSchemaResponse, service_name: &str, graph: &str) {
    if response.service_was_created {
        log::info!(
            "A new service called '{}' for the '{}' graph was created",
            service_name,
            graph
        );
    } else {
        log::info!(
            "The '{}' service for the '{}' graph was updated",
            service_name,
            graph
        );
    }

    if response.did_update_gateway {
        log::info!("The gateway for the '{}' graph was updated with a new schema, composed from the updated '{}' service", graph, service_name);
    } else {
        log::info!(
            "The gateway for the '{}' graph was NOT updated with a new schema",
            graph
        );
    }

    if let Some(errors) = response.composition_errors {
        log::error!(
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
    use super::{handle_response, PushPartialSchemaResponse};

    #[test]
    fn get_schema_from_file_path_loads() {
        // todo @jake -- add test for this after merging with avery's work
    }

    // this test is a bit weird, since we can't test the output. We just verify it
    // doesn't error
    #[test]
    fn handle_response_doesnt_error_with_allsuccesses() {
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
