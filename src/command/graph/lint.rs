use clap::Parser;
use rover_client::operations::graph::lint::{self, LintGraphInput};
use serde::Serialize;

use crate::options::{GraphRefOpt, LintOpts, ProfileOpt, SchemaOpt};

use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Lint {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(flatten)]
    #[serde(skip_serializing)]
    schema: SchemaOpt,

    #[clap(flatten)]
    lint: LintOpts,
}

impl Lint {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let proposed_schema = self
            .schema
            .read_file_descriptor("SDL", &mut std::io::stdin())?;

        let lint_result = lint::run(
            LintGraphInput {
                graph_ref: self.graph.graph_ref.clone(),
                proposed_schema,
                ignore_existing: self.lint.ignore_existing_lint_violations,
            },
            &client,
        )?;

        // TODO: Replace this with real output
        Ok(RoverOutput::SupergraphSchema(lint_result.result))
    }
}
