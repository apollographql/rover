use serde::Serialize;
use structopt::StructOpt;

use rover_client::operations::graph::check::{self, GraphCheckInput};
use rover_client::operations::graph::async_check::{self, CheckSchemaAsyncInput};
use rover_client::shared::{CheckConfig, GitContext, GraphRef, ValidationPeriod};

use crate::command::RoverOutput;
use crate::options::{CheckConfigOpts, GraphRefOpt, ProfileOpt, SchemaOpt};
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Check {
    #[structopt(flatten)]
    graph: GraphRefOpt,

    #[structopt(flatten)]
    profile: ProfileOpt,

    #[structopt(flatten)]
    #[serde(skip_serializing)]
    schema: SchemaOpt,

    #[structopt(flatten)]
    config: CheckConfigOpts,

    /// If the check should be run asynchronously
    #[structopt(long="async", short="a")]
    asynchronous: bool,
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
            "Checking the proposed schema against metrics from {}",
            &self.graph.graph_ref
        );
        if self.asynchronous {
            let res = async_check::run(
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
            Ok(RoverOutput::AsyncCheckResponse(res))
        } else {
            let res = check::run(
                GraphCheckInput {
                    graph_ref: self.graph.graph_ref.clone(),
                    proposed_schema,
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
}
