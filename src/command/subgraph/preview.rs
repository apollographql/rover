use std::collections::BTreeMap;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::{ArgGroup, Parser};
use rover_client::operations::{
    preview_status::{self, PreviewStatusInput},
    subgraph::preview::{
        self, ComposeAndFilterPreviewInput, ContractFilterConfig, SubgraphChange,
        SubgraphChangeInfo,
    },
};
use rover_std::{Fs, Style};
use serde::{Deserialize, Serialize};

use crate::{
    RoverError, RoverOutput, RoverResult,
    options::{OptionalGraphRefOpt, ProfileOpt},
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
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

    /// Preview with these subgraphs hypothetically changed or removed first,
    /// as described by a YAML file (or `-` for stdin).
    ///
    /// The file has a `subgraphs` map keyed by subgraph name, e.g.:
    ///
    ///   subgraphs:
    ///     foo:
    ///       routing_url: https://example.com  # optional; omit to keep the existing URL
    ///       schema:
    ///         file: ./foo.graphql              # or `sdl: "type Query { ... }"` inline
    ///     bar:
    ///       remove: true
    ///
    /// A subgraph entry may set `remove: true` to preview composition as if
    /// the subgraph had been removed.
    #[arg(long = "subgraph-changes", value_name = "FILE", verbatim_doc_comment)]
    #[serde(skip_serializing)]
    subgraph_changes_file: Option<FileDescriptorType>,

    /// List of tag names to include in the previewed contract schema (e.g. '--include-tag foo --include-tag bar').
    /// Omit all of --include-tag/--exclude-tag/--hide-unreachable-types (and their --no-* counterparts)
    /// to preview composition only, with no filtering applied.
    #[arg(long)]
    #[serde(skip_serializing)]
    include_tag: Vec<String>,

    /// Use an empty include list of tag names for the previewed contract schema.
    /// To specify a non-empty list, use --include-tag instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    no_include_tags: bool,

    /// List of tag names to exclude from the previewed contract schema (e.g. '--exclude-tag foo --exclude-tag bar').
    /// To specify an empty list, use --no-exclude-tags instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    exclude_tag: Vec<String>,

    /// Use an empty exclude list of tag names for the previewed contract schema.
    /// To specify a non-empty list, use --exclude-tag instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    no_exclude_tags: bool,

    /// Automatically hide types that can never be reached in operations on the previewed contract schema.
    #[arg(long)]
    #[serde(skip_serializing)]
    hide_unreachable_types: bool,

    /// Do not automatically hide types that can never be reached in operations on the previewed contract schema.
    #[arg(long)]
    #[serde(skip_serializing)]
    no_hide_unreachable_types: bool,

    /// Start the build and return immediately with a job ID, instead of
    /// waiting for it to complete.
    ///
    /// Omit this flag to have Rover poll for the preview to complete.
    /// Polling will timeout after APOLLO_CHECKS_TIMEOUT_SECONDS
    #[arg(long = "async")]
    asynchronous: bool,

    /// Check the status of a previously started job instead of starting a new
    /// one. Checks once, without polling.
    /// TODO: Is it important to allow restarting polling? Or is the current
    /// implementation of either poll from the start or manage async on your own
    /// okay?
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
            "subgraph_changes_file",
        ]
    )]
    job_id: Option<String>,
}

/// The `--subgraph-changes` file format: a `subgraphs` map keyed by subgraph
/// name. Modeled after `supergraph.yaml`'s `subgraphs` map but unlike that
/// format, fields are optional and keep existing values if omitted, has an
/// explicit `remove` field, and `schema` only supports inline SDL or a local
/// file (not a remote URL)
#[derive(Debug, Deserialize)]
struct SubgraphChangesFile {
    subgraphs: BTreeMap<String, SubgraphChangeEntry>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubgraphChangeEntry {
    #[serde(default)]
    remove: bool,
    routing_url: Option<String>,
    schema: Option<SubgraphSchemaSource>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SubgraphSchemaSource {
    File { file: Utf8PathBuf },
    Sdl { sdl: String },
}

impl SubgraphSchemaSource {
    fn read(self, subgraph_name: &str) -> RoverResult<String> {
        match self {
            SubgraphSchemaSource::Sdl { sdl } => Ok(sdl),
            SubgraphSchemaSource::File { file } => Fs::read_file(&file).map_err(|err| {
                RoverError::new(anyhow!(
                    "Could not read schema file for subgraph '{subgraph_name}': {err}"
                ))
            }),
        }
    }
}

impl SubgraphChangeEntry {
    fn into_subgraph_change(self, name: String) -> RoverResult<SubgraphChange> {
        if self.remove {
            if self.routing_url.is_some() || self.schema.is_some() {
                return Err(RoverError::new(anyhow!(
                    "Subgraph '{name}' has `remove: true` but also specifies routing_url/schema — a removed subgraph can't also have changes."
                )));
            }
            return Ok(SubgraphChange { name, info: None });
        }

        let schema_document = self.schema.map(|source| source.read(&name)).transpose()?;

        if self.routing_url.is_none() && schema_document.is_none() {
            return Err(RoverError::new(anyhow!(
                "Subgraph '{name}' in --subgraph-changes must specify at least one of routing_url, schema, or remove: true."
            )));
        }

        Ok(SubgraphChange {
            name,
            info: Some(SubgraphChangeInfo {
                routing_url: self.routing_url,
                schema_document,
            }),
        })
    }
}

impl Preview {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        checks_timeout_seconds: u64,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        if let Some(job_id) = &self.job_id {
            eprintln!(
                "Checking status of subgraph preview job {} using credentials from the {} profile.",
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
            "Previewing composed schema for {} using credentials from the {} profile.",
            Style::Link.paint(graph_ref.to_string()),
            Style::Command.paint(&self.profile.profile_name)
        );
        if !self.asynchronous {
            eprintln!(
                "Waiting for the preview to complete... or press Ctrl+C and check later with {}.",
                Style::Command.paint("`rover subgraph preview --job-id <JOB_ID>`")
            );
        }

        let input = ComposeAndFilterPreviewInput {
            graph_ref,
            filter_config: self.filter_config()?,
            subgraph_changes: self.subgraph_changes()?,
        };
        let preview_response = if self.asynchronous {
            preview::start(input, &client).await?
        } else {
            preview::run(input, &client, checks_timeout_seconds).await?
        };

        Ok(RoverOutput::PreviewJob(preview_response))
    }

    /// Builds the filter config from the paired include/exclude/hide flags.
    /// Omitting all six is allowed (compose-only preview, no filtering).
    /// Supplying any one of the three pairs requires all three.
    fn filter_config(&self) -> RoverResult<Option<ContractFilterConfig>> {
        let no_filter_flags_given = self.include_tag.is_empty()
            && !self.no_include_tags
            && self.exclude_tag.is_empty()
            && !self.no_exclude_tags
            && !self.hide_unreachable_types
            && !self.no_hide_unreachable_types;

        if no_filter_flags_given {
            return Ok(None);
        }

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
        Ok(Some(ContractFilterConfig {
            include: self.include_tag.clone(),
            exclude: self.exclude_tag.clone(),
            hide_unreachable_types: self.hide_unreachable_types,
        }))
    }

    /// Parse `--subgraph-changes` into the list of per-subgraph changes to preview.
    fn subgraph_changes(&self) -> RoverResult<Vec<SubgraphChange>> {
        let Some(file_descriptor) = &self.subgraph_changes_file else {
            return Ok(Vec::new());
        };

        let contents =
            file_descriptor.read_file_descriptor("subgraph changes", &mut std::io::stdin())?;

        let parsed: SubgraphChangesFile = serde_yaml::from_str(&contents)
            .map_err(|err| RoverError::new(anyhow!("Invalid --subgraph-changes file: {err}")))?;

        parsed
            .subgraphs
            .into_iter()
            .map(|(name, entry)| entry.into_subgraph_change(name))
            .collect()
    }
}
