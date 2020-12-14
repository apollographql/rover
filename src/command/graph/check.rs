use std::path::PathBuf;

use anyhow::{Context, Result};
use prettytable::{cell, row, Table};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::graph::check;

use crate::client::StudioClientConfig;
use crate::command::RoverStdout;
use crate::utils::parsers::{parse_graph_ref, GraphRef};

#[derive(Debug, Serialize, StructOpt)]
pub struct Check {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to validate.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF", parse(try_from_str = parse_graph_ref))]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// Path of .graphql/.gql schema file to push
    #[structopt(long = "schema", short = "s")]
    #[serde(skip_serializing)]
    schema_path: PathBuf,
}

impl Check {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let client = client_config.get_client(&self.profile_name)?;
        let schema = std::fs::read_to_string(&self.schema_path)
            .with_context(|| format!("Could not read file `{}`", &self.schema_path.display()))?;
        let res = check::run(
            check::check_schema_query::Variables {
                graph_id: self.graph.name.clone(),
                variant: Some(self.graph.variant.clone()),
                schema: Some(schema),
            },
            &client,
        )
        .context("Failed to validate schema")?;

        tracing::info!(
            "Validated schema against metrics from variant {} on graph {}",
            &self.graph.variant,
            &self.graph.name
        );
        tracing::info!(
            "Compared {} schema changes against {} operations",
            res.changes.len(),
            res.number_of_checked_operations
        );

        if let Some(url) = res.target_url {
            tracing::info!("View full details here");
            tracing::info!("{}", url.to_string());
        }

        let num_failures = print_changes(&res.changes);

        match num_failures {
            0 => Ok(RoverStdout::None),
            1 => Err(anyhow::anyhow!("Encountered 1 failure.")),
            _ => Err(anyhow::anyhow!("Encountered {} failures.", num_failures)),
        }
    }
}

fn print_changes(
    checks: &[check::check_schema_query::CheckSchemaQueryServiceCheckSchemaDiffToPreviousChanges],
) -> u64 {
    let mut num_failures = 0;

    if !checks.is_empty() {
        let mut table = Table::new();
        table.add_row(row!["Change", "Code", "Description"]);
        for check in checks {
            let change = match check.severity {
                check::check_schema_query::ChangeSeverity::NOTICE => "PASS",
                check::check_schema_query::ChangeSeverity::FAILURE => {
                    num_failures += 1;
                    "FAIL"
                }
                _ => unreachable!("Unknown change severity"),
            };
            table.add_row(row![change, check.code, check.description]);
        }

        eprintln!("{}", table);
    }

    num_failures
}
