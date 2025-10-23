use clap::Parser;
use rover_client::{
    operations::graph::publish::{self, GraphPublishInput},
    shared::GitContext,
};
use rover_std::Style;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    options::{GraphRefOpt, ProfileOpt, SchemaOpt},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(flatten)]
    #[serde(skip_serializing)]
    schema: SchemaOpt,
}

impl Publish {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();
        eprintln!(
            "Publishing SDL to {} using credentials from the {} profile.",
            Style::Link.paint(graph_ref),
            Style::Command.paint(&self.profile.profile_name)
        );

        let proposed_schema = self
            .schema
            .read_file_descriptor("SDL", &mut std::io::stdin())?;

        tracing::debug!("Publishing \n{}", &proposed_schema);

        let publish_response = publish::run(
            GraphPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                proposed_schema,
                git_context,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::GraphPublishResponse {
            graph_ref: self.graph.graph_ref.clone(),
            publish_response,
        })
    }
}
