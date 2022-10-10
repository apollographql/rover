use saucer::{clap, Parser};
use serde::Serialize;

use rover_client::operations::graph::delete::{self, GraphDeleteInput};

use crate::command::RoverOutput;
use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::color::Style;
use crate::utils::{self, client::StudioClientConfig};
use crate::Result;

#[derive(Debug, Serialize, Parser)]
pub struct Delete {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// Skips the step where the command asks for user confirmation before
    /// deleting the graph.
    #[clap(long)]
    confirm: bool,
}

impl Delete {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();

        eprintln!(
            "Deleting {} using credentials from the {} profile.",
            Style::Link.paint(&graph_ref),
            Style::Command.paint(&self.profile.profile_name)
        );

        if !self.confirm && !utils::confirm_delete()? {
            eprintln!("Delete cancelled by user");
            return Ok(RoverOutput::EmptySuccess);
        }

        delete::run(
            GraphDeleteInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )?;

        eprintln!("Successfully deleted {}.", Style::Link.paint(&graph_ref));
        Ok(RoverOutput::EmptySuccess)
    }
}
