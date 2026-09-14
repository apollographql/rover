mod output;

use anyhow::anyhow;
use clap::Parser;
use output::GraphPublishLaunchesOutput;
use rover_client::{
    RoverClientError,
    operations::graph::{
        check::{self, CheckSchemaAsyncInput},
        check_workflow::{self, CheckWorkflowInput},
        publish::{self, GraphPublishInput},
    },
    shared::{CheckConfig, GitContext},
};
use rover_print::{
    print::{Print, PrintExt},
    style::{Style, StyledText},
};
use serde::Serialize;

use crate::{
    RoverError, RoverOutput, RoverResult,
    command::CliOutput,
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
        stderr: &impl Print,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let proposed_schema = self
            .schema
            .read_file_descriptor("SDL", &mut std::io::stdin())?;

        if self.check {
            stderr.print(&StyledText::plain(format!(
                "Checking the proposed schema against {}",
                stderr.paint(Style::Link, self.graph.graph_ref.to_string())
            )));

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
                    stderr.print(&StyledText::plain(
                        crate::command::check_output::CheckWorkflowOutput(&check_res).text(),
                    ));
                    stderr.print(&StyledText::new(
                        Style::Success,
                        "Check passed. Publishing SDL",
                    ));
                }
                Err(RoverClientError::CheckWorkflowFailure { check_response, .. }) => {
                    stderr.print(&StyledText::plain(
                        crate::command::check_output::CheckWorkflowOutput(&check_response).text(),
                    ));
                    stderr.print(&StyledText::new(
                        Style::Failure,
                        "Schema check failed — no changes were published to the graph registry.",
                    ));
                    return Err(RoverError::new(anyhow!(
                        "Schema checks must pass before publishing. Fix the check failures above and try again."
                    )));
                }
                Err(e) => {
                    stderr.print(&StyledText::new(
                        Style::Failure,
                        "Schema check failed — no changes were published to the graph registry.",
                    ));
                    return Err(RoverError::new(e));
                }
            }
        }

        stderr.print(&StyledText::plain(format!(
            "Publishing SDL to {} using credentials from the {} profile.",
            stderr.paint(Style::Link, self.graph.graph_ref.to_string()),
            stderr.paint(Style::Command, &self.profile.profile_name)
        )));

        tracing::debug!("Publishing \n{}", &proposed_schema);

        let publish_response = publish::run(
            GraphPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                proposed_schema,
                git_context,
                changelog_message: self.changelog_message.clone(),
            },
            &client,
            checks_timeout_seconds,
        )
        .await?;

        let launches_output = GraphPublishLaunchesOutput(&publish_response);
        let launches_text = launches_output.text();
        if !launches_text.is_empty() {
            stderr.print(&StyledText::plain(
                "Note: a future version of `rover graph publish` will include this report in its stdout output. If you need to reliably parse just the schema hash, use `--format json` and read `.data.api_schema_hash`.".to_string(),
            ));
            stderr.print(&StyledText::plain(launches_text));
        }
        if launches_output.exit_code() != 0 {
            return Err(RoverError::new(anyhow!(
                "The publish succeeded, but a triggered launch did not complete successfully. See the launch report above for details."
            )));
        }

        Ok(RoverOutput::GraphPublishResponse {
            graph_ref: self.graph.graph_ref.clone(),
            publish_response,
        })
    }
}
