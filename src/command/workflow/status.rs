use std::time::{Instant, Duration};

use crossterm::event::poll;
use rover_client::operations::workflow::status::{run, CheckWorkflowInput, types::CheckWorkflowStatus};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::shared::{GraphRef};

use crate::command::workflow::status;
use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Status {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// The id of the workflow to check
    #[structopt(long, short = "i")]
    #[serde(skip_serializing)]
    id: String,

    /// If the command should block and poll until the workflow completes
    #[structopt(long="wait", short="w")]
    wait: bool,
}

impl Status {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        eprintln!(
            "Fetching the status of the check from {}",
            &self.graph
        );

        let res = status::run(
            CheckWorkflowInput {
                graph_ref: self.graph.clone(),
                workflow_id: self.id.clone(),
            },
            &client,
        )?;
        if res.status != CheckWorkflowStatus::PENDING && self.wait {
            eprintln!("Waiting for check to complete...");
            let now = Instant::now();
            let timeout_seconds = 5; // 5 minutes
            let polling_result = loop {
                let output = status::run(
                    CheckWorkflowInput {
                        graph_ref: self.graph.clone(),
                        workflow_id: self.id.clone(),
                    },
                    &client,
                )?;
                let status = output.status;
                if status == CheckWorkflowStatus::PENDING {
                    break RoverOutput::CheckWorkflowResponse(output)
                }
                if now.elapsed() > Duration::from_secs(timeout_seconds) {
                    eprintln!("Timeout after {} seconds waiting for check to complete, check again later.", timeout_seconds);
                    break RoverOutput::EmptySuccess
                }
                std::thread::sleep(Duration::from_secs(5));
            };
            Ok(polling_result)
        } else {
            Ok(RoverOutput::CheckWorkflowResponse(res))
        }
    }
}
