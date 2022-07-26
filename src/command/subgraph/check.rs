use saucer::{clap, Parser};
use serde::Serialize;

use rover_client::operations::subgraph::check::{self, SubgraphCheckInput};
use rover_client::shared::{CheckConfig, GitContext};

use crate::command::RoverOutput;
use crate::options::{CheckConfigOpts, GraphRefOpt, ProfileOpt, SchemaOpt, SubgraphOpt};
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, Parser)]
pub struct Check {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    subgraph: SubgraphOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(flatten)]
    #[serde(skip_serializing)]
    schema: SchemaOpt,

    #[clap(flatten)]
    config: CheckConfigOpts,
}

impl Check {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile.profile_name)?;

        let proposed_schema = self
            .schema
            .read_file_descriptor("SDL", &mut std::io::stdin())?;

        eprintln!(
            "Checking the proposed schema for subgraph {} against {}",
            &self.subgraph.subgraph_name, &self.graph.graph_ref
        );

        let res = check::run(
            SubgraphCheckInput {
                graph_ref: self.graph.graph_ref.clone(),
                proposed_schema,
                subgraph: self.subgraph.subgraph_name.clone(),
                git_context,
                config: CheckConfig {
                    query_count_threshold: self.config.query_count_threshold,
                    query_count_threshold_percentage: self.config.query_percentage_threshold,
                    validation_period: self.config.validation_period.clone(),
                },
            },
            &client,
        )?;

        Ok(RoverOutput::CheckResponse(res))
    }
}
