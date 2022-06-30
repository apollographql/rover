use rover_client::operations::subgraph::check::{self, SubgraphCheckAsyncInput};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::operations::subgraph::check_workflow::{self, CheckWorkflowInput};
use rover_client::shared::{CheckConfig, GitContext};

use crate::command::RoverOutput;
use crate::options::{CheckConfigOpts, GraphRefOpt, ProfileOpt, SchemaOpt, SubgraphOpt};
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Check {
    #[structopt(flatten)]
    graph: GraphRefOpt,

    #[structopt(flatten)]
    subgraph: SubgraphOpt,

    #[structopt(flatten)]
    profile: ProfileOpt,

    #[structopt(flatten)]
    #[serde(skip_serializing)]
    schema: SchemaOpt,

    #[structopt(flatten)]
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

        let workflow_res = check::run(
            SubgraphCheckAsyncInput {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph: self.subgraph.subgraph_name.clone(),
                git_context,
                proposed_schema,
                config: CheckConfig {
                    query_count_threshold: self.config.query_count_threshold,
                    query_count_threshold_percentage: self.config.query_percentage_threshold,
                    validation_period: self.config.validation_period.clone(),
                },
            },
            &client,
        )?;
        if self.config.background {
            Ok(RoverOutput::AsyncCheckResponse(workflow_res))
        } else {
            let check_res = check_workflow::run(
                CheckWorkflowInput {
                    graph_ref: self.graph.graph_ref.clone(),
                    workflow_id: workflow_res.workflow_id,
                },
                self.subgraph.subgraph_name.clone(),
                &client,
            )?;

            Ok(RoverOutput::CheckResponse(check_res))
        }
    }
}
