use anyhow::anyhow;
use clap::{ArgGroup, Parser};
use rover_client::operations::contract::preview::{self, ContractFilterConfig, ContractPreviewInput};
use rover_client::operations::preview_status::{self, PreviewStatusInput};
use rover_std::Style;
use serde::Serialize;

use crate::{
    RoverError, RoverOutput, RoverResult,
    options::{OptionalGraphRefOpt, ProfileOpt},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
#[clap(
    group = ArgGroup::new("include_tags_group")
        .args(&["include_tag", "no_include_tags"]),
    group = ArgGroup::new("exclude_tags_group")
        .args(&["exclude_tag", "no_exclude_tags"]),
    group = ArgGroup::new("hide_unreachable_types_group")
        .args(&["hide_unreachable_types", "no_hide_unreachable_types"])
)]
pub struct Preview {
    /// Required unless --job-id is given (a job's status can be checked
    /// without knowing which graph/variant started it).
    #[clap(flatten)]
    graph: OptionalGraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// List of tag names to include in the contract preview schema (e.g. '--include-tag foo --include-tag bar').
    /// To specify an empty list, use --no-include-tags instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    include_tag: Vec<String>,

    /// Use an empty include list of tag names for the contract preview schema.
    /// To specify a non-empty list, use --include-tag instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    no_include_tags: bool,

    /// List of tag names to exclude from the contract preview schema (e.g. '--exclude-tag foo --exclude-tag bar').
    /// To specify an empty list, use --no-exclude-tags instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    exclude_tag: Vec<String>,

    /// Use an empty exclude list of tag names for the contract preview schema.
    /// To specify a non-empty list, use --exclude-tag instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    no_exclude_tags: bool,

    /// Automatically hide types that can never be reached in operations on the contract preview schema.
    #[arg(long)]
    #[serde(skip_serializing)]
    hide_unreachable_types: bool,

    /// Do not automatically hide types that can never be reached in operations on the contract preview schema.
    #[arg(long)]
    #[serde(skip_serializing)]
    no_hide_unreachable_types: bool,

    /// Start the build and return immediately with a job ID, instead of
    /// waiting for it to complete.
    ///
    /// Every preview build runs asynchronously on the server; without this
    /// flag, Rover starts the build and polls its status every ten seconds
    /// until it finishes. With this flag, Rover only starts the build — check
    /// on it later with --job-id.
    #[arg(long = "async")]
    asynchronous: bool,

    /// Check the status of a previously started job instead of starting a new
    /// one. Checks once, without polling.
    ///
    /// Unlike starting a build, checking its status isn't scoped to a
    /// graph/variant, so <GRAPH_REF> has no effect here — only the job ID
    /// matters.
    #[arg(
        long = "job-id",
        conflicts_with_all = [
            "asynchronous",
            "include_tag",
            "no_include_tags",
            "exclude_tag",
            "no_exclude_tags",
            "hide_unreachable_types",
            "no_hide_unreachable_types",
        ]
    )]
    job_id: Option<String>,
}

impl Preview {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        checks_timeout_seconds: u64,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        // --job-id: check the status of an existing job once, without polling.
        // This is not scoped to a graph/variant (previewStatus is a top-level
        // query keyed only by job ID), so <GRAPH_REF> isn't needed here.
        if let Some(job_id) = &self.job_id {
            eprintln!(
                "Checking status of contract preview job {} using credentials from the {} profile.",
                Style::Link.paint(job_id),
                Style::Command.paint(&self.profile.profile_name)
            );
            let preview_response = preview_status::results(
                PreviewStatusInput {
                    job_id: job_id.clone(),
                },
                &client,
            )
            .await?;
            return Ok(RoverOutput::PreviewJob(preview_response));
        }

        let graph_ref = self.graph.graph_ref.clone().ok_or_else(|| {
            RoverError::new(anyhow!("<GRAPH_REF> is required unless --job-id is given."))
        })?;

        eprintln!(
            "Previewing contract schema for {} using credentials from the {} profile.",
            Style::Link.paint(graph_ref.to_string()),
            Style::Command.paint(&self.profile.profile_name)
        );
        if !self.asynchronous {
            eprintln!(
                "Waiting for the build to complete (checking every 5 seconds)... Press Ctrl+C and check back later with {}.",
                Style::Command.paint("`rover contract preview --job-id <JOB_ID>`")
            );
        }

        // contractPreviewAsync: filter the variant's already-composed
        // supergraph. Filtering is mandatory here.
        let input = ContractPreviewInput {
            graph_ref,
            filter_config: self.required_filter_config()?,
        };
        let preview_response = if self.asynchronous {
            preview::start(input, &client).await?
        } else {
            preview::run(input, &client, checks_timeout_seconds).await?
        };

        Ok(RoverOutput::PreviewJob(preview_response))
    }

    /// Builds the filter config from the paired include/exclude/hide flags,
    /// enforcing that exactly one flag from each pair was provided.
    ///
    /// This is enforced at runtime rather than with `ArgGroup::required(true)`
    /// (as `contract publish` does) because the pairs are not required in
    /// `--job-id` mode.
    fn required_filter_config(&self) -> RoverResult<ContractFilterConfig> {
        if self.include_tag.is_empty() && !self.no_include_tags {
            return Err(RoverError::new(anyhow!(
                "You must specify either --include-tag <TAG> or --no-include-tags."
            )));
        }
        if self.exclude_tag.is_empty() && !self.no_exclude_tags {
            return Err(RoverError::new(anyhow!(
                "You must specify either --exclude-tag <TAG> or --no-exclude-tags."
            )));
        }
        if !self.hide_unreachable_types && !self.no_hide_unreachable_types {
            return Err(RoverError::new(anyhow!(
                "You must specify either --hide-unreachable-types or --no-hide-unreachable-types."
            )));
        }
        Ok(ContractFilterConfig {
            include: self.include_tag.clone(),
            exclude: self.exclude_tag.clone(),
            hide_unreachable_types: self.hide_unreachable_types,
        })
    }
}
