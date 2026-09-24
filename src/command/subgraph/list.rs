use clap::Parser;
use rover_client::operations::subgraph::list::{self, SubgraphListInput};
use rover_std::Style;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    options::{GraphRefOpt, ProfileOpt},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct List {
    #[clap(flatten)]
    graph: GraphRefOpt,
}

impl List {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(profile)?;

        eprintln!(
            "Listing subgraphs for {} using credentials from the {} profile.",
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Link.paint(&profile.profile_name)
        );

        let list_details = list::run(
            SubgraphListInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::SubgraphList(list_details))
    }
}
