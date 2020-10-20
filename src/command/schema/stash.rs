use anyhow::Result;
use houston as config;
use rover_client::blocking::Client;
use rover_client::query::schema::{stash, stash_partial};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Stash {
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
    service_name: Option<String>,
}

impl Stash {
    pub fn run(&self) -> Result<()> {
        match config::Profile::get_api_key(&self.profile) {
            Ok(api_key) => {
                log::info!(
                    "Let's stash this schema, {}@{}, mx. {}!",
                    &self.graph,
                    &self.variant,
                    &self.profile
                );

                let file_contents = get_schema_from_file_path(&self.schema_path);
                let schema_document = match file_contents {
                    Ok(contents) => contents,
                    Err(e) => {
                        // TODO: how can we print this error in a pretty way rather than just returning?
                        // log::error!("{}", e);
                        return Err(e);
                    }
                };

                // TODO (future): move client creation to session
                let client = Client::new(
                    api_key,
                    "https://graphql.api.apollographql.com/api/graphql".to_string(),
                );

                match &self.service_name {
                    Some(service_name) => {
                        let stash_response = stash_partial::run(stash_partial::stash_partial_schema_mutation::Variables {
                            id: self.graph.clone(),
                            graph_variant: self.variant.clone(),
                            name: service_name.clone(),
                            active_partial_schema: stash_partial::stash_partial_schema_mutation::PartialSchemaInput {
                                sdl: Some(schema_document),
                                hash: None
                            },
                            revision: "".to_string(),
                            url: "".to_string(),
                        }, client);

                        match stash_response {
                            Ok(response) => {
                                if response.service_was_created {
                                    log::info!(
                                        "A new service called '{}' for the '{}' graph was created",
                                        service_name,
                                        self.graph
                                    );
                                } else {
                                    log::info!(
                                        "The '{}' service for the '{}' graph was updated",
                                        service_name,
                                        self.graph
                                    );
                                }

                                if response.did_update_gateway {
                                    log::info!("The gateway for the '{}' graph was updated with a new schema, composed from the updated '{}' service", &self.graph, service_name);
                                } else {
                                    log::info!("The gateway for the '{}' graph was NOT updated with a new schema", &self.graph);
                                }

                                if let Some(errors) = response.composition_errors {
                                    log::error!(
                                        "The following composition errors occurred: \n{}",
                                        errors.join("\n")
                                    );
                                }
                            }
                            Err(err) => {
                                log::error!("{}", err);
                            }
                        }
                        Ok(())
                    }
                    None => {
                        let stash_response = stash::run(
                            stash::stash_schema_mutation::Variables {
                                graph_id: self.graph.clone(),
                                variant: self.variant.clone(),
                                schema_document: Some(schema_document),
                            },
                            client,
                        );

                        match stash_response {
                            Ok(response) => {
                                log::info!("{}", response.message);
                                log::info!("Schema Hash: {}", response.schema_hash);
                            }
                            Err(err) => {
                                log::error!("{}", err);
                            }
                        }

                        Ok(())
                    }
                }
            }
            Err(e) => Err(e),
        }
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
