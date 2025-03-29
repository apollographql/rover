use crate::command::subgraph::publish_shared::determine_routing_url;
use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::FileDescriptorType;
use crate::{RoverOutput, RoverResult};
use anyhow::Context;
use clap::Parser;
use rover_client::operations::subgraph::publish_manifest::{
    self, SubgraphManifest, SubgraphsPublishInput,
};
use rover_client::operations::subgraph::routing_url;
use rover_client::operations::subgraph::routing_url::SubgraphRoutingUrlInput;
use rover_client::shared::GitContext;
use rover_std::Style;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct PublishManifest {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// Indicate whether to convert a non-federated graph into a subgraph
    #[arg(short, long)]
    convert: bool,

    #[serde(skip_serializing)]
    #[arg(long)]
    manifest: FileDescriptorType,
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

        let invalid_json_err = |json| format!("JSON in {json} did not match expected format");

        let mut subgraph_manifest = serde_json::from_str::<SubgraphManifest>(&raw_manifest)
            .with_context(|| invalid_json_err(&self.manifest))?;

        // Determine routing urls for all graphs in subgraph_manifest
        for subgraph in subgraph_manifest.subgraph_inputs.iter_mut() {
            subgraph.url = determine_routing_url(
                subgraph.no_url,
                &subgraph.url,
                subgraph.allow_invalid_routing_url,
                || async {
                    Ok(routing_url::run(
                        SubgraphRoutingUrlInput {
                            graph_ref: self.graph.graph_ref.clone(),
                            subgraph_name: subgraph.subgraph.clone(),
                        },
                        &client,
                    )
                    .await?)
                },
            )
            .await?;
        }

        let subgraph_names = subgraph_manifest.get_subgraph_names();
        eprintln!(
            "Publishing SDL to {} (subgraphs: {}) using credentials from the {} profile.",
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Link.paint(subgraph_names.join(", ")),
            Style::Command.paint(&self.profile.profile_name)
        );

        let publish_response = publish_manifest::run(
            SubgraphsPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                git_context,
                subgraph_manifest,
                convert_to_federated_graph: self.convert,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::PublishManifestResponse {
            graph_ref: self.graph.graph_ref.clone(),
            subgraphs: subgraph_names,
            publish_response,
        })
    }
}

#[cfg(test)]
mod tests {}
