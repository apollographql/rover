use clap::Parser;
use serde::Serialize;

use crate::options::{
    DefaultPromptAnswer, GraphRefOpt, ProfileOpt, SubgraphOpt, YesOrNoPromptOpts,
};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::operations::subgraph::delete::{self, SubgraphDeleteInput};
use rover_std::{prompt, Style};

#[derive(Debug, Serialize, Parser)]
pub struct Delete {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    subgraph: SubgraphOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// Skips the step where the command asks for user confirmation before
    /// deleting the subgraph. Also skips preview of build errors that
    /// might occur.
    #[clap(flatten)]
    prompt_opts: YesOrNoPromptOpts,
}

impl Delete {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        // default path previews composition errors that will be caused by
        // a subgraph delete and require confirmation to continue.
        if !self.prompt_opts.yes {
            eprintln!(
                "Checking for build errors resulting from deleting subgraph {} from {} using credentials from the {} profile.",
                Style::Link.paint(&self.subgraph.subgraph_name),
                Style::Link.paint(self.graph.graph_ref.to_string()),
                Style::Command.paint(&self.profile.profile_name)
            );

            let dry_run = true;
            // run delete with dryRun, so we can preview build errors
            let delete_dry_run_response = delete::run(
                SubgraphDeleteInput {
                    graph_ref: self.graph.graph_ref.clone(),
                    subgraph: self.subgraph.subgraph_name.clone(),
                    dry_run,
                },
                &client,
            )?;

            let delete_response = RoverOutput::SubgraphDeleteResponse {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph: self.subgraph.subgraph_name.clone(),
                dry_run,
                delete_response: delete_dry_run_response,
            }
            .get_stdout()?;

            self.prompt_opts.prompt(
                &format!(
                    "Are you sure you want to delete {subgraph} from {graph}?",
                    subgraph = &self.subgraph.subgraph_name,
                    graph = &self.graph.graph_ref
                ),
                DefaultPromptAnswer::No,
                "Subgraph deletion",
            )?;
        }

        let dry_run = false;

        let delete_response = delete::run(
            SubgraphDeleteInput {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph: self.subgraph.subgraph_name.clone(),
                dry_run,
            },
            &client,
        )?;

        Ok(RoverOutput::SubgraphDeleteResponse {
            graph_ref: self.graph.graph_ref.clone(),
            subgraph: self.subgraph.subgraph_name.clone(),
            dry_run,
            delete_response,
        })
    }
}
