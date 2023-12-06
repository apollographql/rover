use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use clap::Parser;
use rover_client::operations::license::fetch::LicenseFetchInput;
use rover_std::Style;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct Fetch {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Fetch {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        eprintln!(
            "Fetching license for {} using credentials from the {} profile.",
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Command.paint(&self.profile.profile_name)
        );
        let jwt = rover_client::operations::license::fetch::run(
            LicenseFetchInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )?;

        Ok(RoverOutput::LicenseResponse {
            graph_ref: self.graph.graph_ref.clone(),
            jwt,
        })
    }
}
