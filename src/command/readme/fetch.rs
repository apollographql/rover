use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;
use rover_client::operations::readme::fetch::{self, ReadmeFetchInput};
use rover_client::shared::GraphRef;

use ansi_term::Colour::{Cyan, Yellow};

#[derive(Debug, Serialize, StructOpt)]
pub struct Fetch {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to publish to.
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
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let graph_ref = self.graph.to_string();

        eprintln!(
            "Fetching graph variant README of {} using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile_name)
        );
        let readme = fetch::run(
            ReadmeFetchInput {
                graph_ref: self.graph.clone(),
            },
            &client,
        )?;
        Ok(RoverOutput::ReadmeFetchResponse(readme))
    }
}
