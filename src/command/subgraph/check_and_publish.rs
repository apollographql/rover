use std::io::{self, IsTerminal};

use anyhow::anyhow;
use clap::Parser;
use rover_client::{
    RoverClientError,
    operations::subgraph::{
        check::{self, SubgraphCheckAsyncInput},
        check_workflow::{self, CheckWorkflowInput},
        publish::{self, SubgraphPublishInput},
        routing_url::{self, SubgraphRoutingUrlInput},
    },
    shared::{CheckConfig, GitContext},
};
use rover_std::Style;
use serde::Serialize;

use crate::{
    RoverError, RoverOutput, RoverResult,
    options::{CheckConfigOpts, GraphRefOpt, ProfileOpt, SchemaOpt, SubgraphOpt},
    utils::client::StudioClientConfig,
};

use super::publish::Publish;

#[derive(Debug, Serialize, Parser)]
pub struct CheckAndPublish {
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

    /// Indicate whether to convert a non-federated graph into a subgraph
    #[arg(short, long)]
    convert: bool,

    /// Url of a running subgraph that a supergraph can route operations to
    #[arg(long)]
    #[serde(skip_serializing)]
    routing_url: Option<String>,

    /// Bypasses warnings and the prompt to confirm publish when the routing url is invalid
    #[arg(long)]
    allow_invalid_routing_url: bool,

    /// Shorthand for `--routing-url "" --allow-invalid-routing-url`
    #[arg(long)]
    no_url: bool,
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

        // Resolve routing URL upfront so we don't prompt the user after a failed check.
        let url = Publish::determine_routing_url(
            self.no_url,
            &self.routing_url,
            self.allow_invalid_routing_url,
            || async {
                Ok(routing_url::run(
                    SubgraphRoutingUrlInput {
                        graph_ref: self.graph.graph_ref.clone(),
                        subgraph_name: self.subgraph.subgraph_name.clone(),
                    },
                    &client,
                )
                .await?)
            },
            &mut io::stderr(),
            &mut io::stdin(),
            io::stderr().is_terminal() && io::stdin().is_terminal(),
        )
        .await?;

        eprintln!(
            "Checking the proposed schema for subgraph {} against {}",
            Style::Link.paint(&self.subgraph.subgraph_name),
            Style::Link.paint(self.graph.graph_ref.to_string())
        );

        let workflow_res = check::run(
            SubgraphCheckAsyncInput {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph: self.subgraph.subgraph_name.clone(),
                git_context: git_context.clone(),
                proposed_schema: proposed_schema.clone(),
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
        let check_res = match check_workflow::run(
            CheckWorkflowInput {
                graph_ref: self.graph.graph_ref.clone(),
                workflow_id: workflow_res.workflow_id,
                checks_timeout_seconds,
            },
            self.subgraph.subgraph_name.clone(),
            &client,
        )
        .await
        {
            Ok(res) => res,
            Err(RoverClientError::CheckWorkflowFailure {
                check_response, ..
            }) => {
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
            Err(e) => return Err(RoverError::new(e)),
        };

        eprintln!("{}", check_res.get_output());
        eprintln!(
            "Check passed. Publishing SDL to {} (subgraph: {}) using credentials from the {} profile.",
            Style::Link.paint(self.graph.graph_ref.to_string()),
            Style::Link.paint(&self.subgraph.subgraph_name),
            Style::Command.paint(&self.profile.profile_name)
        );

        let publish_response = publish::run(
            SubgraphPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph: self.subgraph.subgraph_name.clone(),
                url,
                schema: proposed_schema,
                git_context,
                convert_to_federated_graph: self.convert,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::SubgraphPublishResponse {
            graph_ref: self.graph.graph_ref.clone(),
            subgraph: self.subgraph.subgraph_name.clone(),
            publish_response,
        })
    }
}
