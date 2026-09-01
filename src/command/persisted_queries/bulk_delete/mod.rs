mod output;

use std::{str::FromStr, time::Duration};

use clap::Parser;
use output::BulkDeleteOutput;
use rover_client::{
    RoverClientError,
    blocking::StudioClient,
    operations::persisted_queries::{
        bulk_deletion_status::{
            self, BulkDeletionJobStatus, BulkDeletionStatus, BulkDeletionStatusInput,
            BulkDeletionStatusResponse,
        },
        start_bulk_deletion::{
            self, PersistedQueryDeletionFilter, PersistedQueryId, StartBulkDeletion,
            StartBulkDeletionInput,
        },
    },
};
use rover_print::print::PrintExt;
use serde::Serialize;
use tower::{Service, ServiceExt};

use super::identify::identify_persisted_query_list;
use crate::{
    RoverOutput, RoverResult,
    options::{OptionalGraphRefOpt, ProfileOpt},
    utils::client::StudioClientConfig,
};

/// How long to wait between polls of `bulkDeletionStatus`. Deliberately not
/// user-configurable: the CLI has no chunking or retry policy of its own, so the
/// only knobs that matter are the durable submit/poll API and this progress cadence.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// An operation ID paired with the optional client name that distinguishes it, parsed
/// from `ID` or `ID:CLIENT_NAME` (see `BulkDelete::exclude`'s doc for why the client
/// name matters).
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
struct ExcludeOperation {
    id: String,
    client_name: Option<String>,
}

impl FromStr for ExcludeOperation {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.split_once(':') {
            Some((id, client_name)) => ExcludeOperation {
                id: id.to_string(),
                client_name: Some(client_name.to_string()),
            },
            None => ExcludeOperation {
                id: s.to_string(),
                client_name: None,
            },
        })
    }
}

/// Start (or resume watching) an asynchronous bulk deletion job against a Persisted
/// Query List.
///
/// Deletion happens server-side: Rover submits the job and polls for its status.
/// Interrupting this command (e.g. with Ctrl-C) does not cancel the job — resume
/// watching it later with `--job-id`.
#[derive(Debug, Serialize, Parser)]
pub struct BulkDelete {
    #[clap(flatten)]
    graph: OptionalGraphRefOpt,

    /// The Graph ID the list to delete from belongs to.
    #[serde(skip_serializing)]
    #[arg(long, conflicts_with = "graph_ref")]
    graph_id: Option<String>,

    /// The list ID to delete operations from.
    #[serde(skip_serializing)]
    #[arg(long, conflicts_with = "graph_ref")]
    list_id: Option<String>,

    /// Resume watching an already-started bulk deletion job instead of starting a new one.
    #[arg(
        long,
        conflicts_with_all = ["client_name", "name_contains", "published_after", "published_before", "exclude", "no_wait"]
    )]
    job_id: Option<String>,

    /// Only delete operations whose client name is one of these. May be passed multiple times.
    #[arg(long = "client-name")]
    client_name: Vec<String>,

    /// Only delete operations whose name contains this case-insensitive substring.
    #[arg(long)]
    name_contains: Option<String>,

    /// Only delete operations last published at or after this RFC 3339 timestamp.
    #[arg(long)]
    published_after: Option<String>,

    /// Only delete operations last published at or before this RFC 3339 timestamp.
    #[arg(long)]
    published_before: Option<String>,

    /// An operation to exclude from deletion, even if it matches the filter, given as
    /// `ID` or `ID:CLIENT_NAME`. Operations are identified by id AND client name, not
    /// id alone, so a bare ID only excludes the operation published with no client
    /// name; if the one you mean to protect has a client name, include it
    /// (`id:client-name`) or it will be deleted despite being "excluded". May be
    /// passed multiple times.
    #[arg(long = "exclude", value_name = "ID[:CLIENT_NAME]")]
    exclude: Vec<ExcludeOperation>,

    /// Submit the bulk deletion job and print its job ID immediately, without
    /// waiting for it to finish.
    #[arg(long)]
    no_wait: bool,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl BulkDelete {
    pub async fn run<P: PrintExt>(
        &self,
        client_config: StudioClientConfig,
        stderr: &P,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let (graph_id, list_id, list_name) = identify_persisted_query_list(
            &self.graph,
            &self.graph_id,
            &self.list_id,
            "deleting operations from",
            &client,
        )
        .await?;

        let job_id = match &self.job_id {
            Some(job_id) => job_id.clone(),
            None => {
                self.submit(&client, &graph_id, &list_id, &list_name, stderr)
                    .await?
            }
        };

        if self.no_wait {
            return Ok(RoverOutput::CliOutput(Box::new(
                BulkDeleteOutput::Submitted { job_id },
            )));
        }

        self.poll(&client, &graph_id, &list_id, job_id, list_name, stderr)
            .await
    }

    /// Submits a new bulk deletion job for the filter/exclude criteria given on the
    /// command line, and returns its job ID.
    async fn submit<P: PrintExt>(
        &self,
        client: &StudioClient,
        graph_id: &str,
        list_id: &str,
        list_name: &str,
        stderr: &P,
    ) -> RoverResult<String> {
        let filter = PersistedQueryDeletionFilter {
            clients: (!self.client_name.is_empty()).then(|| self.client_name.clone()),
            name_contains: self.name_contains.clone(),
            last_published_after: self.published_after.clone(),
            last_published_before: self.published_before.clone(),
        };
        let exclude = self
            .exclude
            .iter()
            .map(|exclude| PersistedQueryId {
                id: exclude.id.clone(),
                client_name: exclude.client_name.clone(),
            })
            .collect();

        let inner = client
            .studio_graphql_service_with_timeout(start_bulk_deletion::DEFAULT_ATTEMPT_TIMEOUT)?;
        let mut service = StartBulkDeletion::new(inner);
        let response = service
            .ready()
            .await?
            .call(StartBulkDeletionInput {
                graph_id: graph_id.to_string(),
                list_id: list_id.to_string(),
                filter,
                exclude,
            })
            .await?;

        stderr.infoln(format!(
            "Started bulk deletion job {} for list {} ({}).",
            response.job_id, list_name, graph_id
        ));

        Ok(response.job_id)
    }

    /// Polls `bulkDeletionStatus` until the job reaches a terminal state, printing
    /// progress as it goes.
    async fn poll<P: PrintExt>(
        &self,
        client: &StudioClient,
        graph_id: &str,
        list_id: &str,
        job_id: String,
        list_name: String,
        stderr: &P,
    ) -> RoverResult<RoverOutput> {
        let inner = client
            .studio_graphql_service_with_timeout(bulk_deletion_status::DEFAULT_ATTEMPT_TIMEOUT)?;
        let mut service = BulkDeletionStatus::new(inner);

        loop {
            let status = service
                .ready()
                .await?
                .call(BulkDeletionStatusInput {
                    graph_id: graph_id.to_string(),
                    list_id: list_id.to_string(),
                    job_id: job_id.clone(),
                })
                .await?;

            match status {
                BulkDeletionStatusResponse::Pending {
                    status,
                    operations_deleted_so_far,
                } => {
                    let status = match status {
                        BulkDeletionJobStatus::Pending => "queued",
                        BulkDeletionJobStatus::Running => "running",
                    };
                    stderr.infoln(format!(
                        "{operations_deleted_so_far} operations deleted so far ({status})..."
                    ));
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
                BulkDeletionStatusResponse::Success {
                    revision,
                    list_name: latest_list_name,
                } => {
                    return Ok(RoverOutput::CliOutput(Box::new(
                        BulkDeleteOutput::Success {
                            job_id,
                            // Prefer the name straight off this poll: it's always
                            // current, whereas `list_name` was captured before the
                            // job ran (or in a prior invocation, when resuming via
                            // --job-id) and could be stale if the list was renamed
                            // meanwhile. It's `None` only when nothing changed (no
                            // build was produced), in which case the earlier name
                            // is the best one available.
                            list_name: latest_list_name.unwrap_or(list_name),
                            revision,
                        },
                    )));
                }
                BulkDeletionStatusResponse::Failure { error } => {
                    return Err(RoverClientError::BulkDeletionJobFailed { job_id, error }.into());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn exclude_operation_parses_a_bare_id_with_no_client_name() {
        let parsed: ExcludeOperation = "abc123".parse().unwrap();
        assert_that!(parsed).is_equal_to(ExcludeOperation {
            id: "abc123".to_string(),
            client_name: None,
        });
    }

    #[test]
    fn exclude_operation_parses_an_id_and_client_name() {
        let parsed: ExcludeOperation = "abc123:web".parse().unwrap();
        assert_that!(parsed).is_equal_to(ExcludeOperation {
            id: "abc123".to_string(),
            client_name: Some("web".to_string()),
        });
    }

    #[test]
    fn exclude_operation_keeps_a_colon_containing_client_name_intact() {
        let parsed: ExcludeOperation = "abc123:studio:web".parse().unwrap();
        assert_that!(parsed).is_equal_to(ExcludeOperation {
            id: "abc123".to_string(),
            client_name: Some("studio:web".to_string()),
        });
    }
}
