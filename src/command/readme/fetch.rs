use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::Result;

use rover_client::operations::readme::fetch::{self, ReadmeFetchInput};

use ansi_term::Colour::{Cyan, Yellow};

#[derive(Debug, Serialize, StructOpt)]
pub struct Fetch {
    #[structopt(flatten)]
    graph: GraphRefOpt,

    #[structopt(flatten)]
    profile: ProfileOpt,
}

impl Fetch {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile.profile_name)?;
        let graph_ref = self.graph.graph_ref.to_string();

        eprintln!(
            "Fetching README for {} using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile.profile_name)
        );
        let readme = fetch::run(
            ReadmeFetchInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )?;
        Ok(RoverOutput::ReadmeFetchResponse {
            graph_ref: self.graph.graph_ref.clone(),
            content: readme.content,
            last_updated_time: readme.last_updated_time,
        })
    }
}
