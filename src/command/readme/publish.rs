use clap::Parser;
use rover_client::operations::readme::publish::{self, ReadmePublishInput};
use rover_std::Style;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    options::{GraphRefOpt, ProfileOpt},
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
};

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// The file upload as the README. You can pass `-` to use stdin instead of a file.
    #[arg(long, short = 's')]
    #[serde(skip_serializing)]
    file: FileDescriptorType,
}

impl Publish {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();
        eprintln!(
            "Publishing README for {} using credentials from the {} profile.",
            Style::Link.paint(graph_ref),
            Style::Command.paint(&self.profile.profile_name)
        );

        let new_readme = self
            .file
            .read_file_descriptor("README", &mut std::io::stdin())?;
        tracing::debug!("Uploading \n{}", &new_readme);

        let publish_response = publish::run(
            ReadmePublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                readme: new_readme,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::ReadmePublishResponse {
            graph_ref: self.graph.graph_ref.clone(),
            new_content: publish_response.new_content,
            last_updated_time: publish_response.last_updated_time,
        })
    }
}
