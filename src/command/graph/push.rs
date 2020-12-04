use anyhow::{Context, Result};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::schema::push;

use crate::client::get_studio_client;
use crate::command::RoverStdout;
use crate::utils::loaders::load_schema_from_flag;
use crate::utils::parsers::{parse_schema_location, SchemaLocation};

#[derive(Debug, Serialize, StructOpt)]
pub struct Push {
    /// The schema file to push
    /// Can pass `-` to use stdin instead of a file
    #[structopt(long, short = "s", parse(from_str = parse_schema_location))]
    #[serde(skip_serializing)]
    schema: SchemaLocation,

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

        let schema_document = load_schema_from_flag(&self.schema)?;

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
    use super::{handle_response, push};

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
