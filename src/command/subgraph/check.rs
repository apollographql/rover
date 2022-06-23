use serde::Serialize;
use structopt::StructOpt;

use rover_client::operations::subgraph::check::{self, SubgraphCheckInput};
use rover_client::shared::{CheckConfig, GitContext, ValidationPeriod};

use crate::command::RoverOutput;
use crate::options::{GraphRefOpt, ProfileOpt, SchemaOpt, SubgraphOpt};
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::{parse_query_count_threshold, parse_query_percentage_threshold};
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

    /// The minimum number of times a query or mutation must have been executed
    /// in order to be considered in the check operation
    #[structopt(long, parse(try_from_str = parse_query_count_threshold))]
    query_count_threshold: Option<i64>,

    /// Minimum percentage of times a query or mutation must have been executed
    /// in the time window, relative to total request count, for it to be
    /// considered in the check. Valid numbers are in the range 0 <= x <= 100
    #[structopt(long, parse(try_from_str = parse_query_percentage_threshold))]
    query_percentage_threshold: Option<f64>,

    /// Size of the time window with which to validate schema against (i.e "24h" or "1w 2d 5h")
    #[structopt(long)]
    validation_period: Option<ValidationPeriod>,
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
                    query_count_threshold: self.query_count_threshold,
                    query_count_threshold_percentage: self.query_percentage_threshold,
                    validation_period: self.validation_period.clone(),
                },
            },
            &client,
        )?;

        Ok(RoverOutput::CheckResponse(res))
    }
}
