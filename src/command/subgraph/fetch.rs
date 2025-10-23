use clap::Parser;
use rover_client::operations::subgraph::fetch::{self, SubgraphFetchInput};
use rover_std::Style;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    options::{GraphRefOpt, ProfileOpt, SubgraphOpt},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct Fetch {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    subgraph: SubgraphOpt,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Fetch {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();
        eprintln!(
            "Fetching SDL from {} (subgraph: {}) using credentials from the {} profile.",
            Style::Link.paint(graph_ref),
            Style::Link.paint(&self.subgraph.subgraph_name),
            Style::Command.paint(&self.profile.profile_name)
        );

        let fetch_response = fetch::run(
            SubgraphFetchInput {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph_name: self.subgraph.subgraph_name.clone(),
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::FetchResponse(fetch_response))
    }
}
