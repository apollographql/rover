use crate::utils::client::StudioClientConfig;
use crate::{
    options::{GraphRefOpt, ProfileOpt},
    RoverOutput, RoverResult,
};

use rover_client::operations::supergraph::fetch::{self, SupergraphFetchInput};
use rover_std::{Spinner, Style};

use clap::Parser;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct Fetch {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Fetch {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();
        let spinner = Spinner::new(&format!(
            "Fetching supergraph SDL from {} using credentials from the {} profile.",
            Style::GraphRef.paint(graph_ref),
            Style::Command.paint(&self.profile.profile_name)
        ));

        let fetch_response = fetch::run(
            SupergraphFetchInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )
        .await?;

        spinner.stop();

        Ok(RoverOutput::FetchResponse(fetch_response))
    }
}
