use anyhow::Result;
use houston as config;
use rover_client::blocking::Client;
use rover_client::query::schema::stash;
use structopt::StructOpt;
use std::path::Path;

#[derive(Debug, StructOpt)]
pub struct Stash {
    /// where to find the schema. .graphql, .json or uri
    #[structopt(name = "SCHEMA_PATH")]
    schema_path: String,
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

                // TODO (future): move client creation to session
                let client = Client::new(
                    api_key,
                    "https://graphql.api.apollographql.com/api/graphql".to_string(),
                );

                let path = Path::new(&self.schema_path);
                let contents = std::fs::read_to_string(path);
                let schema_document = match contents {
                    Ok(schema) => {
                        schema
                    }, 
                    Err(e) => {
                        panic!("Unable to open file: {} [ERROR]: {}", path.display(), e);
                    }
                };

                let stash_response = stash::run(
                    stash::stash_schema_mutation::Variables {
                        graph_id: self.graph.clone(),
                        variant: self.variant.clone(),
                        schema_document: Some(schema_document),
                    },
                    client,
                );

                match stash_response {
                    Ok((message, hash)) => {
                        log::info!("{}", message);
                        log::info!("Schema Hash: {}", hash);
                    },
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
