use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::operations::graph::list::{self, GraphListInput};
use rover_client::shared::GraphRef;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct List {
    /// <NAME> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> must be left off
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl List {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let graph_ref = self.graph.to_string();
        eprintln!(
            "Listing variants from {} using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile_name)
        );

        let list_response = list::run(
            GraphListInput {
                graph_ref: self.graph.clone(),
            },
            &client,
        )?;

        Ok(RoverOutput::GraphList(list_response))
    }
}
