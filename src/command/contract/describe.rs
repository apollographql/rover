use clap::Parser;
use rover_client::operations::contract::describe::{self, ContractDescribeInput};
use rover_std::Style;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    options::{GraphRefOpt, ProfileOpt},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct Describe {
    #[clap(flatten)]
    graph: GraphRefOpt,
}

impl Describe {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(profile)?;
        eprintln!(
            "Fetching description for configuration of {} using credentials from the {} profile.\n",
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Command.paint(&profile.profile_name)
        );

        let describe_response = describe::run(
            ContractDescribeInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::ContractDescribe(describe_response))
    }
}
