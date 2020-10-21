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

    #[structopt(long)]
    graph: String,

    #[structopt(long, default_value = "default")]
    profile: String,

    /// for federated graphs, what service is to be updated
    #[structopt(long)]
    service_name: String,
}

impl Push {
    pub fn run(&self) -> Result<()> {
        let client = get_rover_client(&self.profile)?;

        log::info!(
            "Let's push this schema, {}@{}, mx. {}!",
            &self.graph,
            &self.variant,
            &self.profile
        );

        let schema_document = get_schema_from_file_path(&self.schema_path)?;

        let push_response = push::run(
            push::push_partial_schema_mutation::Variables {
                id: self.graph.clone(),
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

        handle_response(push_response, &self.service_name, &self.graph);
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
