use crate::client::get_rover_client;
use anyhow::Result;
use rover_client::query::schema::push;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Push {
    /// Path to a .graphql SDL file
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
            push::push_schema_mutation::Variables {
                graph_id: self.graph_name.clone(),
                variant: self.variant.clone(),
                schema_document: Some(schema_document),
            },
            client,
        )?;

        log::info!(
            "{}\nSchema Hash: {}",
            push_response.message,
            push_response.schema_hash
        );
        Ok(())
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

    #[test]
    fn get_schema_from_file_path_loads() {
        // todo @jake -- add test for this after merging with avery's work
    }
}
