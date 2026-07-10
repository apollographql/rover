use anyhow::anyhow;
use clap::Parser;
use rover_client::{
    RoverClientError,
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
    RoverError, RoverOutput, RoverResult,
    options::{CheckConfigOpts, GraphRefOpt, ProfileOpt, SchemaOpt},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(flatten)]
    #[serde(skip_serializing)]
    schema: SchemaOpt,

    /// Run schema checks before publishing and abort if they fail
    #[arg(long)]
    check: bool,

    #[clap(flatten)]
    check_config: CheckConfigOpts,

    /// A message to associate with this publish in the Studio changelog
    #[arg(long, value_name = "MESSAGE")]
    changelog_message: Option<String>,
}

impl Publish {
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

        if self.check {
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
                        validation_period: self.check_config.validation_period.clone(),
                        query_count_threshold: self.check_config.query_count_threshold,
                        query_count_threshold_percentage: self
                            .check_config
                            .query_percentage_threshold,
                    },
                },
                &client,
            )
            .await?;

            match check_workflow::run(
                CheckWorkflowInput {
                    graph_ref: self.graph.graph_ref.clone(),
                    workflow_id: workflow_res.workflow_id,
                    checks_timeout_seconds,
                },
                &client,
            )
            .await
            {
                Ok(check_res) => {
                    eprintln!("{}", check_res.get_output());
                    eprintln!("{}", Style::Success.paint("Check passed. Publishing SDL"));
                }
                Err(RoverClientError::CheckWorkflowFailure { check_response, .. }) => {
                    eprintln!("{}", check_response.get_output());
                    eprintln!(
                        "{}",
                        Style::Failure.paint(
                            "Schema check failed — no changes were published to the graph registry."
                        )
                    );
                    return Err(RoverError::new(anyhow!(
                        "Schema checks must pass before publishing. Fix the check failures above and try again."
                    )));
                }
                Err(e) => {
                    eprintln!(
                        "{}",
                        Style::Failure.paint(
                            "Schema check failed — no changes were published to the graph registry."
                        )
                    );
                    return Err(RoverError::new(e));
                }
            }
        }

        eprintln!(
            "Publishing SDL to {} using credentials from the {} profile.",
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Command.paint(&self.profile.profile_name)
        );

        tracing::debug!("Publishing \n{}", &proposed_schema);

        let publish_response = publish::run(
            GraphPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                proposed_schema,
                git_context,
                changelog_message: self.changelog_message.clone(),
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
