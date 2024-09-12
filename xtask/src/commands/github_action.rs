use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::Parser;
use octocrab::{models::RunId, Octocrab, OctocrabBuilder};
use serde_json::json;

const WORKFLOW_GET_ID_TIMEOUT: Duration = Duration::from_secs(30);
const WORKFLOW_RUN_TIMEOUT: Duration = Duration::from_secs(600);
const WORKFLOW_WAIT_TIME: Duration = Duration::from_secs(2);

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

    /// The repository branch or tag to use
    #[arg(long = "git-ref", default_value = "main")]
    pub(crate) git_ref: String,

    /// The commit ID for this run
    #[arg(long = "commit-id")]
    pub(crate) commit_id: String,

    /// A JSON document to use as inputs for GitHub Actions
    #[arg(long = "inputs")]
    pub(crate) inputs: Option<String>,
}

impl GithubActions {
    pub async fn run(&self) -> Result<()> {
        let git_ref = if self.git_ref.is_empty() {
            String::from("main")
        } else {
            self.git_ref.clone()
        };
        crate::info!("Running against git ref {}", git_ref);
        let token = std::env::var("GITHUB_ACTIONS_TOKEN")
            .map_err(|_err| anyhow!("$GITHUB_ACTIONS_TOKEN is not set or is not valid UTF-8."))?;
        let octocrab = OctocrabBuilder::new()
            .personal_token(token.clone())
            .build()?;

        // Find information about the current user
        let user = octocrab.current().user().await?.login;

        // Trigger GitHub workflow by sending a workflow dispatch event
        // See <https://docs.github.com/en/rest/actions/workflows?apiVersion=2022-11-28#create-a-workflow-dispatch-event>
        let mut json_payload = json!({"ref": git_ref});
        if let Some(inputs) = &self.inputs {
            let json_inputs: serde_json::Value = serde_json::from_str(inputs)?;
            json_payload = json!({
                "ref": git_ref,
                "inputs": json_inputs,
            });
        }

        let res = octocrab
            ._post(
                format!(
                    "https://api.github.com/repos/{}/{}/actions/workflows/{}/dispatches",
                    self.organization, self.repository, self.workflow_name
                ),
                Some(&json_payload),
            )
            .await?;

        if !res.status().is_success() {
            return Err(anyhow!(
                "failed to start workflow, got status code {}",
                res.status()
            ));
        }

        // Find the corresponding workflow run ID
        let fut = async {
            loop {
                match self.get_run_id(&octocrab, &user, &git_ref).await {
                    Ok(run_id) => return run_id,
                    Err(_err) => {
                        tokio::time::sleep(WORKFLOW_WAIT_TIME).await;
                    }
                }
            }
        };
        let run_id = tokio::time::timeout(WORKFLOW_GET_ID_TIMEOUT, fut).await?;

        crate::info!("monitoring run {}", run_id);

        match self.check_run(&octocrab, run_id).await {
            Ok(()) => crate::info!("run {} completed successfully", run_id),
            Err(_err) => crate::info!("run {} failed or did not complete in time", run_id),
        }

        Ok(())
    }

    async fn get_run_id(&self, octocrab: &Octocrab, login: &str, git_ref: &str) -> Result<RunId> {
        Ok(octocrab
            .workflows(&self.organization, &self.repository)
            .list_runs(&self.workflow_name)
            // Despite the name, this does appear to work with tags.
            .branch(git_ref)
            .event("workflow_dispatch")
            .actor(login)
            .send()
            .await?
            .into_iter()
            .find(|run| run.head_commit.id == self.commit_id)
            .ok_or_else(|| anyhow!("could not find a matching run on GitHub"))?
            .id)
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
                    _ => {
                        tokio::time::sleep(WORKFLOW_WAIT_TIME).await;
                    }
                }
            }
        };

        tokio::time::timeout(WORKFLOW_RUN_TIMEOUT, fut).await?
    }
}
