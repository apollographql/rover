use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::options::{GraphRefOpt, ProfileOpt, SubgraphOpt};
use crate::utils::{self, client::StudioClientConfig};
use crate::Result;

use rover_client::operations::subgraph::delete::{self, SubgraphDeleteInput};

#[derive(Debug, Serialize, StructOpt)]
pub struct Delete {
    #[structopt(flatten)]
    graph: GraphRefOpt,

    #[structopt(flatten)]
    subgraph: SubgraphOpt,

    #[structopt(flatten)]
    profile: ProfileOpt,

    /// Skips the step where the command asks for user confirmation before
    /// deleting the subgraph. Also skips preview of build errors that
    /// might occur
    #[structopt(long)]
    confirm: bool,
}

impl Delete {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile.profile_name)?;
        eprintln!(
            "Checking for build errors resulting from deleting subgraph {} from {} using credentials from the {} profile.",
            Cyan.normal().paint(&self.subgraph.subgraph_name),
            Cyan.normal().paint(self.graph.graph_ref.to_string()),
            Yellow.normal().paint(&self.profile.profile_name)
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
            )?;

            RoverOutput::SubgraphDeleteResponse {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph: self.subgraph.subgraph_name.clone(),
                dry_run,
                delete_response: delete_dry_run_response,
            }
            .print()?;

            // I chose not to error here, since this is a perfectly valid path
            if !utils::confirm_delete()? {
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
        )?;

        Ok(RoverOutput::SubgraphDeleteResponse {
            graph_ref: self.graph.graph_ref.clone(),
            subgraph: self.subgraph.subgraph_name.clone(),
            dry_run,
            delete_response,
        })
    }
}
