use clap::Parser;
use rover_client::operations::graph::delete::{self, GraphDeleteInput};
use rover_std::{Style, prompt};
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    options::{GraphRefOpt, ProfileOpt},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct Delete {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// Skips the step where the command asks for user confirmation before
    /// deleting the graph.
    #[arg(long)]
    confirm: bool,
}

impl Delete {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();

        eprintln!(
            "Deleting {} using credentials from the {} profile.",
            Style::Link.paint(&graph_ref),
            Style::Command.paint(&self.profile.profile_name)
        );

        if !self.confirm && !prompt::confirm_delete()? {
            eprintln!("Delete cancelled by user");
            return Ok(RoverOutput::EmptySuccess);
        }

        delete::run(
            GraphDeleteInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )
        .await?;

        eprintln!("Successfully deleted {}.", Style::Link.paint(&graph_ref));
        Ok(RoverOutput::EmptySuccess)
    }
}
