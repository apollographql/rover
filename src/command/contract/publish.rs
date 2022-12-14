use clap::Parser;
use rover_std::Style;
use serde::Serialize;

use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use rover_client::operations::contract::publish::{self, ContractPublishInput};

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    #[clap(flatten)]
    contract: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Publish {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let contract_ref = self.contract.graph_ref.to_string();

        eprintln!("");
        eprintln!(
            "Checking existence of graph {} using credentials from the {} profile.",
            Style::Link.paint(&contract_ref),
            Style::Command.paint(&self.profile.profile_name)
        );
        eprintln!("");

        let command_result = publish::run(
            ContractPublishInput {
                contract_ref: self.contract.graph_ref.clone(),
            },
            &client,
        )?;
        println!("Graph Name: {:?}", command_result);

        Ok(RoverOutput::EmptySuccess)
    }
}
