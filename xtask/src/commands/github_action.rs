use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::Parser;
use octocrab::{models::RunId, Octocrab, OctocrabBuilder};
use serde_json::json;

const WORKFLOW_RUN_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Parser)]
pub struct GithubActions {
    /// The GitHub workflow name
    #[arg(long = "workflow-name", env = "WORKFLOW_NAME")]
    pub(crate) workflow_name: String,

    /// GitHub organization name
    #[arg(long = "organization", default_value = "apollographql")]
    pub(crate) organization: String,

    /// GitHub repository name
    #[arg(long = "repository", default_value = "rover")]
    pub(crate) repository: String,

    /// The repository branch to use
    #[arg(long = "branch")]
    pub(crate) branch: String,

    /// The commit ID for this run
    #[arg(long = "commit-id")]
    pub(crate) commit_id: String,

    /// A JSON document to use as inputs for GitHub Actions
    #[arg(long = "inputs")]
    pub(crate) inputs: String,
}

impl GithubActions {
    pub async fn run(&self) -> Result<()> {
        let github_token = std::env::var("GITHUB_TOKEN")
            .map_err(|_err| anyhow!("$GITHUB_TOKEN is not set or is not valid UTF-8."))?;
        let octocrab = OctocrabBuilder::new()
            .personal_token(github_token.clone())
            .build()?;

        // Trigger GitHub workflow by sending a workflow dispatch event
        // See <https://docs.github.com/en/rest/actions/workflows?apiVersion=2022-11-28#create-a-workflow-dispatch-event>
        let inputs: serde_json::Value = serde_json::from_str(&self.inputs)?;
        let res = octocrab
            ._post(
                format!(
                    "https://api.github.com/repos/{}/{}/actions/workflows/{}/dispatches",
                    self.organization, self.repository, self.workflow_name
                ),
                Some(&json!({
                    "ref": self.branch,
                    "inputs": inputs,
                })),
            )
            .await?;

        if !res.status().is_success() {
            return Err(anyhow!(
                "failed to start workflow, got status code {}",
                res.status()
            ));
        }

        // Find the corresponding workflow run ID
        let run_id = octocrab
            .workflows(&self.organization, &self.repository)
            .list_runs(&self.workflow_name)
            .branch(&self.branch)
            .event("workflow_dispatch")
            .send()
            .await?
            .into_iter()
            .find(|run| run.head_commit.id == self.commit_id)
            .ok_or_else(|| anyhow!("could not find a matching run on GitHub"))?
            .id;

        self.check_run(&octocrab, run_id).await
    }

    async fn check_run(&self, octocrab: &Octocrab, run_id: RunId) -> Result<()> {
        let fut = async {
            loop {
                let run = octocrab
                    .workflows(&self.organization, &self.repository)
                    .get(run_id)
                    .await?;

                match run.status.as_str() {
                    "completed" => return Ok(()),
                    "failure" => return Err(anyhow!("GitHub workflow run failed")),
                    _ => (),
                }
            }
        };

        tokio::select! {
            _ = tokio::time::sleep(WORKFLOW_RUN_TIMEOUT) => Err(anyhow!("checking workflow run timed out")),
            res = fut => res,

        }
    }
}
