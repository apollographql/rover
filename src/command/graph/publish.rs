use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::operations::graph::publish::{self, GraphPublishInput};
use rover_client::shared::{GitContext, GraphRef};

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::{parse_file_descriptor, FileDescriptorType};
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Publish {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to publish to.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// The schema file to publish. You can pass `-` to use stdin instead of a file.
    #[structopt(long, short = "s", parse(try_from_str = parse_file_descriptor))]
    #[serde(skip_serializing)]
    schema: FileDescriptorType,

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

        let proposed_schema = self
            .schema
            .read_file_descriptor("SDL", &mut std::io::stdin())?;

        tracing::debug!("Publishing \n{}", &proposed_schema);

        let publish_response = publish::run(
            GraphPublishInput {
                graph_ref: self.graph.clone(),
                proposed_schema,
                git_context,
            },
            &client,
        )?;

        Ok(RoverOutput::GraphPublishResponse {
            graph_ref: self.graph.clone(),
            publish_response,
        })
    }
}
