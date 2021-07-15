use crate::utils::client::StudioClientConfig;
use crate::{command::RoverStdout, Result};

use rover_client::operations::supergraph::fetch::{self, SupergraphFetchInput};
use rover_client::shared::GraphRef;

use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Fetch {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Fetch {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let graph_ref = self.graph.to_string();
        eprintln!(
            "Fetching supergraph SDL from {} using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile_name)
        );

        let fetch_response = fetch::run(
            SupergraphFetchInput {
                graph_ref: self.graph.clone(),
            },
            &client,
        )?;

        Ok(RoverStdout::FetchResponse(fetch_response))
    }
}
