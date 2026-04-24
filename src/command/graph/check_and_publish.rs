use clap::Parser;
use rover_client::{
    operations::graph::{
        check::{self, CheckSchemaAsyncInput},
        check_workflow::{self, CheckWorkflowInput},
        publish::{self, GraphPublishInput},
    },
    shared::{CheckConfig, GitContext},
};
use rover_std::Style;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    options::{CheckConfigOpts, GraphRefOpt, ProfileOpt, SchemaOpt},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct CheckAndPublish {
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

impl CheckAndPublish {
    pub async fn run(
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
            "Checking the proposed schema against {}",
            Style::Link.paint(self.graph.graph_ref.to_string())
        );

        let workflow_res = check::run(
            CheckSchemaAsyncInput {
                graph_ref: self.graph.graph_ref.clone(),
                proposed_schema: proposed_schema.clone(),
                git_context: git_context.clone(),
                config: CheckConfig {
                    validation_period: self.config.validation_period.clone(),
                    query_count_threshold: self.config.query_count_threshold,
                    query_count_threshold_percentage: self.config.query_percentage_threshold,
                },
            },
            &client,
        )
        .await?;

        // Always wait for check results before deciding whether to publish.
        let check_res = check_workflow::run(
            CheckWorkflowInput {
                graph_ref: self.graph.graph_ref.clone(),
                workflow_id: workflow_res.workflow_id,
                checks_timeout_seconds,
            },
            &client,
        )
        .await?;

        eprintln!("{}", check_res.get_output());
        eprintln!(
            "Check passed. Publishing SDL to {} using credentials from the {} profile.",
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Command.paint(&self.profile.profile_name)
        );

        let publish_response = publish::run(
            GraphPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                proposed_schema,
                git_context,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::GraphPublishResponse {
            graph_ref: self.graph.graph_ref.clone(),
            publish_response,
        })
    }
}
