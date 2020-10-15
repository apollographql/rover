use anyhow::Result;
use houston as config;
use rover_client::blocking::Client;
use rover_client::query::schema::stash;
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
