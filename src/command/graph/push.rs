use anyhow::{Context, Result};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::schema::push;

use crate::client::get_studio_client;
use crate::command::RoverStdout;
use crate::utils::loaders::load_schema_from_flag;
use crate::utils::parsers::{parse_graph_ref, parse_schema_source, GraphRef, SchemaSource};

#[derive(Debug, Serialize, StructOpt)]
pub struct Push {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to push to.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF", parse(try_from_str = parse_graph_ref))]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// The schema file to push
    /// Can pass `-` to use stdin instead of a file
    #[structopt(long, short = "s", parse(try_from_str = parse_schema_source))]
    #[serde(skip_serializing)]
    schema: SchemaSource,

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
            &self.graph.name,
            &self.graph.variant,
            &self.profile_name
        );

        let schema_document = load_schema_from_flag(&self.schema, std::io::stdin())?;

        tracing::debug!("Schema Document to push:\n{}", &schema_document);

        let push_response = push::run(
            push::push_schema_mutation::Variables {
                graph_id: self.graph.name.clone(),
                variant: self.graph.variant.clone(),
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
