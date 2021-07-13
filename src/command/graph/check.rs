use serde::Serialize;
use structopt::StructOpt;

use rover_client::operations::graph::check::{self, GraphCheckInput};
use rover_client::shared::{CheckConfig, GitContext, GraphRef};

use crate::command::RoverStdout;
use crate::utils::client::StudioClientConfig;
use crate::utils::loaders::load_schema_from_flag;
use crate::utils::parsers::{
    parse_query_count_threshold, parse_query_percentage_threshold, parse_schema_source,
    parse_validation_period, SchemaSource, ValidationPeriod,
};
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Check {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to validate.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// The schema file to check
    /// Can pass `-` to use stdin instead of a file
    #[structopt(long, short = "s", parse(try_from_str = parse_schema_source))]
    #[serde(skip_serializing)]
    schema: SchemaSource,

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
    #[structopt(long, parse(try_from_str = parse_validation_period))]
    validation_period: Option<ValidationPeriod>,
}

impl Check {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> Result<RoverStdout> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let proposed_schema = load_schema_from_flag(&self.schema, std::io::stdin())?;

        eprintln!(
            "Checking the proposed schema against metrics from {}",
            &self.graph
        );

        let res = check::run(
            GraphCheckInput {
                graph_ref: self.graph.clone(),
                proposed_schema,
                git_context,
                config: CheckConfig {
                    query_count_threshold: self.query_count_threshold,
                    query_count_threshold_percentage: self.query_percentage_threshold,
                    validation_period_from: self.validation_period.clone().unwrap_or_default().from,
                    validation_period_to: self.validation_period.clone().unwrap_or_default().to,
                },
            },
            &client,
        )?;

        Ok(RoverStdout::CheckResponse(res))
    }
}
