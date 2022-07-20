use ansi_term::Colour::{Cyan, Yellow};
use saucer::{clap, Parser};
use serde::Serialize;

use rover_client::operations::subgraph::fetch::{self, SubgraphFetchInput};

use crate::command::RoverOutput;
use crate::options::{GraphRefOpt, ProfileOpt, SubgraphOpt};
use crate::utils::client::StudioClientConfig;
use crate::Result;

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
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile.profile_name)?;
        let graph_ref = self.graph.graph_ref.to_string();
        eprintln!(
            "Fetching SDL from {} (subgraph: {}) using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Cyan.normal().paint(&self.subgraph.subgraph_name),
            Yellow.normal().paint(&self.profile.profile_name)
        );

        let fetch_response = fetch::run(
            SubgraphFetchInput {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph_name: self.subgraph.subgraph_name.clone(),
            },
            &client,
        )?;

        Ok(RoverOutput::FetchResponse(fetch_response))
    }
}
