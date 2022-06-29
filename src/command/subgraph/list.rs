use ansi_term::Colour::Cyan;
use serde::Serialize;
use structopt::StructOpt;

use rover_client::operations::subgraph::list::{self, SubgraphListInput};

use crate::command::RoverOutput;
use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct List {
    #[structopt(flatten)]
    graph: GraphRefOpt,

    #[structopt(flatten)]
    profile: ProfileOpt,
}

impl List {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile.profile_name)?;

        eprintln!(
            "Listing subgraphs for {} using credentials from the {} profile.",
            Cyan.normal().paint(self.graph.graph_ref.to_string()),
            Cyan.normal().paint(&self.profile.profile_name)
        );

        let list_details = list::run(
            SubgraphListInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )?;

        Ok(RoverOutput::SubgraphList(list_details))
    }
}
