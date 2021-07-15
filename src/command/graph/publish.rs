use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::operations::graph::publish::{self, GraphPublishInput, GraphPublishResponse};
use rover_client::shared::{GitContext, GraphRef};

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::utils::loaders::load_schema_from_flag;
use crate::utils::parsers::{parse_schema_source, SchemaSource};
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Publish {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to publish to.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// The schema file to publish
    /// Can pass `-` to use stdin instead of a file
    #[structopt(long, short = "s", parse(try_from_str = parse_schema_source))]
    #[serde(skip_serializing)]
    schema: SchemaSource,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Publish {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let graph_ref = self.graph.to_string();
        eprintln!(
            "Publishing SDL to {} using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile_name)
        );

        let proposed_schema = load_schema_from_flag(&self.schema, std::io::stdin())?;

        tracing::debug!("Publishing \n{}", &proposed_schema);

        let publish_response = publish::run(
            GraphPublishInput {
                graph_ref: self.graph.clone(),
                proposed_schema,
                git_context,
            },
            &client,
        )?;

        let hash = handle_response(&self.graph, publish_response);
        Ok(RoverOutput::SchemaHash(hash))
    }
}

/// handle all output logging from operation
fn handle_response(graph: &GraphRef, response: GraphPublishResponse) -> String {
    eprintln!(
        "{}#{} Published successfully {}",
        graph, response.schema_hash, response.change_summary
    );

    response.schema_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_response_doesnt_err() {
        let expected_hash = "123456".to_string();
        let graph = GraphRef {
            name: "harambe".to_string(),
            variant: "inside-job".to_string(),
        };
        let actual_hash = handle_response(
            &graph,
            GraphPublishResponse {
                schema_hash: expected_hash.clone(),
                change_summary: "".to_string(),
            },
        );
        assert_eq!(actual_hash, expected_hash);
    }
}
