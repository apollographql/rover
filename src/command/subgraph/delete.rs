use clap::Parser;
use serde::Serialize;

use crate::options::{GraphRefOpt, ProfileOpt, SubgraphOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::operations::subgraph::delete::{self, SubgraphDeleteInput};
use rover_std::{Style, prompt};

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
    /// might occur
    #[arg(long)]
    confirm: bool,
}

impl Delete {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        eprintln!(
            "Checking for build errors resulting from deleting subgraph {} from {} using credentials from the {} profile.",
            Style::Link.paint(&self.subgraph.subgraph_name),
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Command.paint(&self.profile.profile_name)
        );

        // this is probably the normal path -- preview a subgraph delete
        // and make the user confirm it manually.
        if !self.confirm {
            let dry_run = true;
            // run delete with dryRun, so we can preview build errors
            let delete_dry_run_response = delete::run(
                SubgraphDeleteInput {
                    graph_ref: self.graph.graph_ref.clone(),
                    subgraph: self.subgraph.subgraph_name.clone(),
                    dry_run,
                },
                &client,
            )
            .await?;

            RoverOutput::SubgraphDeleteResponse {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph: self.subgraph.subgraph_name.clone(),
                dry_run,
                delete_response: delete_dry_run_response,
            }
            .get_stdout()?;

            // I chose not to error here, since this is a perfectly valid path
            if !prompt::confirm_delete()? {
                eprintln!("Delete cancelled by user");
                return Ok(RoverOutput::EmptySuccess);
            }
        }

        let dry_run = false;

        let delete_response = delete::run(
            SubgraphDeleteInput {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph: self.subgraph.subgraph_name.clone(),
                dry_run,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::SubgraphDeleteResponse {
            graph_ref: self.graph.graph_ref.clone(),
            subgraph: self.subgraph.subgraph_name.clone(),
            dry_run,
            delete_response,
        })
    }
}
