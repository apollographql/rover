use clap::Parser;
use serde::Serialize;

use rover_client::operations::subgraph::list::{self, SubgraphListInput};
use rover_std::Style;

use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct List {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl List {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        eprintln!(
            "Listing subgraphs for {} using credentials from the {} profile.",
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Link.paint(&self.profile.profile_name)
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
