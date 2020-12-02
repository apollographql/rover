use std::io::{self, Read};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::schema::push;

use crate::client::get_studio_client;
use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Push {
    /// Path of .graphql/.gql schema file to push
    /// Can pass `-` to use stdin instead of a file
    #[structopt(long, short = "s")]
    #[serde(skip_serializing)]
    schema: String,

    /// Name of graph variant in Apollo Studio to push to
    #[structopt(long, default_value = "current")]
    #[serde(skip_serializing)]
    variant: String,

    /// ID of graph in Apollo Studio to push to
    #[structopt(long)]
    #[serde(skip_serializing)]
    graph_name: String,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
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

        let schema_document = if &self.schema == "-" {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        } else {
            get_schema_from_file_path(&self.schema).context("Failed while loading from SDL file")?
        };

        tracing::debug!("Schema Document to push:\n{}", &schema_document);

        let push_response = push::run(
            push::push_schema_mutation::Variables {
                graph_id: self.graph_name.clone(),
                variant: self.variant.clone(),
                schema_document: Some(schema_document),
            },
            &client,
        )
        .context("Failed while pushing to Apollo Studio. To see a full printout of the schema attempting to push, rerun with `--log debug`")?;

        let hash = handle_response(push_response);
        Ok(RoverStdout::SchemaHash(hash))
    }
}

fn get_schema_from_file_path(path: &str) -> Result<String> {
    let path = Path::new(path);
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

/// handle all output logging from operation
fn handle_response(response: push::PushResponse) -> String {
    tracing::info!(
        "{}\nSchema Hash:",
        response.message, // the message will say if successful, and details
    );
    response.schema_hash
}

#[cfg(test)]
mod tests {
    use super::{get_schema_from_file_path, handle_response, push};
    use assert_fs::TempDir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn get_schema_from_file_path_loads() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = "./schema.graphql";
        let mut temp_file = File::create(file_path.clone()).unwrap();
        write!(temp_file, "type Query {{ hello: String! }}").unwrap();

        let schema = get_schema_from_file_path(&file_path).unwrap();
        assert_eq!(schema, "type Query { hello: String! }".to_string());
    }

    #[test]
    fn get_schema_from_file_path_errs_on_bad_path() {
        let empty_path = "./wow.graphql";
        let schema = get_schema_from_file_path(&empty_path);
        assert_eq!(schema.is_err(), true);
    }

    #[test]
    fn handle_response_doesnt_err() {
        let expected_hash = "123456".to_string();
        let actual_hash = handle_response(push::PushResponse {
            message: "oooh wowo it pushed successfully!".to_string(),
            schema_hash: expected_hash.clone(),
        });
        assert_eq!(actual_hash, expected_hash);
    }
}
