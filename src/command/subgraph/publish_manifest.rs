use anyhow::Context;
use clap::Parser;
use serde::{Serialize};
use rover_client::operations::subgraph::publish_manifest::{self, SubgraphsPublishInput, SubgraphManifest};
use rover_client::shared::GitContext;
use rover_std::Style;
use crate::{RoverOutput, RoverResult};
use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::FileDescriptorType;

#[derive(Debug, Serialize, Parser)]
pub struct PublishManifest {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    #[serde(skip_serializing)]
    #[arg(long)]
    manifest: FileDescriptorType
}

impl PublishManifest {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let raw_manifest = self
            .manifest
            .read_file_descriptor("subgraphs list", &mut std::io::stdin())?;

        let invalid_json_err = |json| {
            format!("JSON in {json} did not match expected format")
        };

        let subgraph_manifest =
            serde_json::from_str::<SubgraphManifest>(&raw_manifest)
                .with_context(|| invalid_json_err(&self.manifest))?;

        let subgraph_names = subgraph_manifest.get_subgraph_names();

        eprintln!(
            "Publishing SDL to {} (subgraphs: {}) using credentials from the {} profile.",
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Link.paint(&subgraph_names.join(", ")),
            Style::Command.paint(&self.profile.profile_name)
        );

        let publish_response = publish_manifest::run(
            SubgraphsPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph_manifest,
                git_context,
            },
            &client
        ).await?;

        Ok(RoverOutput::PublishManifestResponse {
            graph_ref: self.graph.graph_ref.clone(),
            subgraphs: subgraph_names,
            publish_response,
        })



        // eprintln!(
        //     "Publishing SDL to {} (subgraph: {}) using credentials from the {} profile.",
        //     Style::Link.paint(self.graph.graph_ref.to_string()),
        //     Style::Link.paint(&self.subgraphs.subgraph_name),
        //     Style::Command.paint(&self.profile.profile_name)
        // );

    }
}