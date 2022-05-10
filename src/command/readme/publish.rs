use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::{parse_file_descriptor, FileDescriptorType};
use crate::Result;
use rover_client::operations::readme::publish::{self, ReadmePublishInput};
use rover_client::shared::GraphRef;

use ansi_term::Colour::{Cyan, Yellow};

#[derive(Debug, Serialize, StructOpt)]
pub struct Publish {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to publish to.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// The file upload as the README. You can pass `-` to use stdin instead of a file.
    #[structopt(long, short = "s", parse(try_from_str = parse_file_descriptor))]
    #[serde(skip_serializing)]
    file: FileDescriptorType,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Publish {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let graph_ref = self.graph.to_string();
        eprintln!(
            "Publishing graph variant README of {} using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile_name)
        );

        let new_readme = self
            .file
            .read_file_descriptor("README", &mut std::io::stdin())?;
        tracing::debug!("Uploading \n{}", &new_readme);

        let publish_response = publish::run(
            ReadmePublishInput {
                graph_ref: self.graph.clone(),
                readme: new_readme,
            },
            &client,
        )?;

        Ok(RoverOutput::ReadmePublishResponse {
            new_content: publish_response.new_content,
            last_updated_at: publish_response.last_updated_at,
        })
    }
}
