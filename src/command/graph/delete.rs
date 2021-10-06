use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::operations::graph::delete::{self, GraphDeleteInput};
use rover_client::shared::GraphRef;

use crate::command::RoverOutput;
use crate::utils::{self, client::StudioClientConfig};
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Delete {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// Skips the step where the command asks for user confirmation before
    /// deleting the graph.
    #[structopt(long)]
    confirm: bool,
}

impl Delete {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let graph_ref = self.graph.to_string();

        eprintln!(
            "Deleting {} using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile_name)
        );

        if !self.confirm && !utils::confirm_delete()? {
            eprintln!("Delete cancelled by user");
            return Ok(RoverOutput::EmptySuccess);
        }

        delete::run(
            GraphDeleteInput {
                graph_ref: self.graph.clone(),
            },
            &client,
        )?;

        eprintln!("Successfully deleted {}.", Cyan.normal().paint(&graph_ref));
        Ok(RoverOutput::EmptySuccess)
    }
}
