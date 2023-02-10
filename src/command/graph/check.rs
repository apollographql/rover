use clap::Parser;
use serde::Serialize;

use rover_client::shared::{CheckConfig, GitContext};
use rover_client::{
    operations::graph::{
        check::{self, CheckSchemaAsyncInput},
        check_workflow::{self, CheckWorkflowInput},
    },
    shared::CheckResponse,
};

use crate::options::{CheckConfigOpts, GraphRefOpt, ProfileOpt, SchemaOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Check {
    #[clap(flatten)]
    graph: GraphRefOpt,

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
        checks_timeout_seconds: u64,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let proposed_schema = self
            .schema
            .read_file_descriptor("SDL", &mut std::io::stdin())?;

        eprintln!(
            "Checking the proposed schema against metrics from {}",
            &self.graph.graph_ref
        );
        let workflow_res = check::run(
            CheckSchemaAsyncInput {
                graph_ref: self.graph.graph_ref.clone(),
                proposed_schema,
                git_context,
                config: CheckConfig {
                    validation_period: self.config.validation_period.clone(),
                    query_count_threshold: self.config.query_count_threshold,
                    query_count_threshold_percentage: self.config.query_percentage_threshold,
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
                    checks_timeout_seconds,
                },
                &client,
            )?;
            Ok(RoverOutput::CheckResponse(
                CheckResponse::OperationCheckResponse(check_res),
            ))
        }
    }
}
