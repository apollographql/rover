use clap::Parser;
use serde::Serialize;

use rover_client::operations::graph::delete::{self, GraphDeleteInput};
use rover_std::Style;

use crate::options::{DefaultPromptAnswer, GraphRefOpt, ProfileOpt, YesOrNoPromptOpts};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Delete {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// Skips the step where the command asks for user confirmation before
    /// deleting the graph.
    #[clap(flatten)]
    prompt_opts: YesOrNoPromptOpts,
}

impl Delete {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();

        self.prompt_opts.prompt(
            &format!(
                "Are you sure you want to delete {graph}?",
                graph = Style::Link.paint(&graph_ref)
            ),
            DefaultPromptAnswer::No,
            "Graph deletion",
        )?;

        eprintln!(
            "Deleting {} using credentials from the {} profile.",
            Style::Link.paint(&graph_ref),
            Style::Command.paint(&self.profile.profile_name)
        );

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
