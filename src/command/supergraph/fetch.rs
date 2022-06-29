use crate::utils::client::StudioClientConfig;
use crate::{
    command::RoverOutput,
    options::{GraphRefOpt, ProfileOpt},
    Result,
};

use rover_client::operations::supergraph::fetch::{self, SupergraphFetchInput};

use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Fetch {
    #[structopt(flatten)]
    graph: GraphRefOpt,

    #[structopt(flatten)]
    profile: ProfileOpt,
}

impl Fetch {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile.profile_name)?;
        let graph_ref = self.graph.graph_ref.to_string();
        eprintln!(
            "Fetching supergraph SDL from {} using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile.profile_name)
        );

        let fetch_response = fetch::run(
            SupergraphFetchInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )?;

        Ok(RoverOutput::FetchResponse(fetch_response))
    }
}
