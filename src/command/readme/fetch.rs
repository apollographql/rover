use clap::Parser;
use rover_std::Style;
use serde::Serialize;

use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::operations::readme::fetch::{self, ReadmeFetchInput};

#[derive(Debug, Serialize, Parser)]
pub struct Fetch {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Fetch {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();

        eprintln!(
            "Fetching README for {} using credentials from the {} profile.",
            Style::Link.paint(graph_ref),
            Style::Command.paint(&self.profile.profile_name)
        );
        let readme = fetch::run(
            ReadmeFetchInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::ReadmeFetchResponse {
            graph_ref: self.graph.graph_ref.clone(),
            content: readme.content,
            last_updated_time: readme.last_updated_time,
        })
    }
}
