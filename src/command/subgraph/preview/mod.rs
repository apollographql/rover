mod output;

use std::collections::BTreeMap;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::{ArgGroup, Parser};
use output::SubgraphPreviewOutput;
use rover_client::operations::subgraph::preview::{
    self, ComposeAndFilterPreviewInput, ComposeAndFilterPreviewStatusInput, ContractFilterConfig,
    SubgraphChange, SubgraphChangeInfo,
};
use rover_print::{
    print::{Print, PrintExt},
    style::{Style, StyledText},
};
use rover_std::Fs;
use serde::{Deserialize, Serialize};

use crate::{
    RoverError, RoverOutput, RoverResult,
    options::{GraphRefOpt, ProfileOpt},
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
    /// Required to identify graph and check permissions even though buildId is unique.
    #[clap(flatten)]
    graph: GraphRefOpt,

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
    #[arg(
        long = "build-id",
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
    build_id: Option<String>,
}

/// The `--subgraph-changes` file format: a `subgraphs` map keyed by subgraph
/// name. Modeled after `supergraph.yaml`'s `subgraphs` map but unlike that
/// format, fields are optional and keep existing values if omitted, has an
/// explicit `remove` field, and `schema` only supports inline SDL or a local
/// file (not a remote URL)
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
        stderr: &impl Print,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.clone();

        if let Some(build_id) = &self.build_id {
            stderr.print(&StyledText::plain(format!(
                "Checking status of subgraph preview job {} on {} using credentials from the {} profile.",
                stderr.paint(Style::Link, build_id),
                stderr.paint(Style::Link, graph_ref.to_string()),
                stderr.paint(Style::Command, &self.profile.profile_name)
            )));
            let preview_response = preview::result(
                ComposeAndFilterPreviewStatusInput {
                    graph_ref,
                    build_id: build_id.clone(),
                },
                &client,
            )
            .await?;
            return Ok(RoverOutput::CliOutput(Box::new(SubgraphPreviewOutput(
                preview_response,
            ))));
        }

        stderr.print(&StyledText::plain(format!(
            "Previewing composed schema for {} using credentials from the {} profile.",
            stderr.paint(Style::Link, graph_ref.to_string()),
            stderr.paint(Style::Command, &self.profile.profile_name)
        )));

        let input = ComposeAndFilterPreviewInput {
            graph_ref: graph_ref.clone(),
            filter_config: self.filter_config()?,
            subgraph_changes: self.subgraph_changes()?,
        };
        let started = preview::start(input, &client).await?;

        if self.asynchronous {
            return Ok(RoverOutput::CliOutput(Box::new(SubgraphPreviewOutput(
                started,
            ))));
        }

        stderr.print(&StyledText::plain(format!(
            "Waiting for the preview to complete... or press Ctrl+C and check later with {}.",
            stderr.paint(
                Style::Command,
                format!(
                    "`rover subgraph preview {} --build-id {}`",
                    graph_ref, started.build_id
                )
            )
        )));

        let preview_response = preview::poll(
            ComposeAndFilterPreviewStatusInput {
                graph_ref,
                build_id: started.build_id,
            },
            &client,
            checks_timeout_seconds,
        )
        .await?;

        Ok(RoverOutput::CliOutput(Box::new(SubgraphPreviewOutput(
            preview_response,
        ))))
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

#[cfg(test)]
mod tests {
    use assert_fs::prelude::*;

    use super::*;

    /// A `Preview` with no filter flags and no `--subgraph-changes` file, the
    /// "compose only" baseline. Individual tests override one field at a
    /// time to exercise each validation branch.
    fn valid_preview() -> Preview {
        Preview {
            graph: GraphRefOpt {
                graph_ref: "test-graph@current".parse().unwrap(),
            },
            profile: ProfileOpt::default(),
            subgraph_changes_file: None,
            include_tag: Vec::new(),
            no_include_tags: false,
            exclude_tag: Vec::new(),
            no_exclude_tags: false,
            hide_unreachable_types: false,
            no_hide_unreachable_types: false,
            asynchronous: false,
            build_id: None,
        }
    }

    #[test]
    fn filter_config_is_none_when_no_flags_are_given() {
        assert_eq!(valid_preview().filter_config().unwrap(), None);
    }

    #[test]
    fn filter_config_builds_config_when_all_three_pairs_are_decided() {
        let preview = Preview {
            include_tag: vec!["foo".to_string()],
            no_exclude_tags: true,
            hide_unreachable_types: true,
            ..valid_preview()
        };
        let config = preview.filter_config().unwrap().unwrap();
        assert_eq!(config.include, vec!["foo".to_string()]);
        assert_eq!(config.exclude, Vec::<String>::new());
        assert!(config.hide_unreachable_types);
    }

    #[test]
    fn filter_config_errors_when_only_some_of_the_pairs_are_decided() {
        // --no-include-tags is decided, but exclude/hide are left ambiguous;
        // supplying any one flag requires all three to be decided.
        let preview = Preview {
            no_include_tags: true,
            ..valid_preview()
        };
        let err = preview.filter_config().unwrap_err();
        assert!(err.to_string().contains("--exclude-tag"));
    }

    #[test]
    fn subgraph_changes_is_empty_without_a_file() {
        assert_eq!(valid_preview().subgraph_changes().unwrap(), Vec::new());
    }

    /// Writes `contents` to a temp `--subgraph-changes` file and returns a
    /// `Preview` pointed at it. The `TempDir` must outlive the returned
    /// `Preview`'s use, hence returning both.
    fn preview_with_changes_file(contents: &str) -> (assert_fs::TempDir, Preview) {
        let fixture = assert_fs::TempDir::new().unwrap();
        let file = fixture.child("subgraph-changes.yaml");
        file.write_str(contents).unwrap();
        let path = Utf8PathBuf::try_from(file.path().to_path_buf()).unwrap();
        let preview = Preview {
            subgraph_changes_file: Some(FileDescriptorType::File(path)),
            ..valid_preview()
        };
        (fixture, preview)
    }

    #[test]
    fn subgraph_changes_parses_inline_sdl_and_routing_url() {
        let (_fixture, preview) = preview_with_changes_file(
            "subgraphs:\n  foo:\n    routing_url: https://example.com\n    schema:\n      sdl: \"type Query { hello: String }\"\n",
        );

        let changes = preview.subgraph_changes().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "foo");
        let info = changes[0].info.as_ref().unwrap();
        assert_eq!(info.routing_url, Some("https://example.com".to_string()));
        assert_eq!(
            info.schema_document,
            Some("type Query { hello: String }".to_string())
        );
    }

    #[test]
    fn subgraph_changes_reads_schema_from_a_file_path() {
        let fixture = assert_fs::TempDir::new().unwrap();
        let schema_file = fixture.child("foo.graphql");
        schema_file.write_str("type Query { hi: String }").unwrap();

        let changes_file = fixture.child("subgraph-changes.yaml");
        changes_file
            .write_str(&format!(
                "subgraphs:\n  foo:\n    schema:\n      file: {}\n",
                schema_file.path().display()
            ))
            .unwrap();
        let path = Utf8PathBuf::try_from(changes_file.path().to_path_buf()).unwrap();
        let preview = Preview {
            subgraph_changes_file: Some(FileDescriptorType::File(path)),
            ..valid_preview()
        };

        let changes = preview.subgraph_changes().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes[0].info.as_ref().unwrap().schema_document,
            Some("type Query { hi: String }".to_string())
        );
    }

    #[test]
    fn subgraph_changes_parses_remove() {
        let (_fixture, preview) =
            preview_with_changes_file("subgraphs:\n  bar:\n    remove: true\n");

        let changes = preview.subgraph_changes().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "bar");
        assert_eq!(changes[0].info, None);
    }

    #[test]
    fn subgraph_changes_rejects_remove_combined_with_routing_url() {
        let (_fixture, preview) = preview_with_changes_file(
            "subgraphs:\n  bar:\n    remove: true\n    routing_url: https://example.com\n",
        );

        let err = preview.subgraph_changes().unwrap_err();
        assert!(err.to_string().contains("remove: true"));
    }

    #[test]
    fn subgraph_changes_rejects_an_entry_with_no_fields_set() {
        let (_fixture, preview) = preview_with_changes_file("subgraphs:\n  bar: {}\n");

        let err = preview.subgraph_changes().unwrap_err();
        assert!(err.to_string().contains("must specify at least one of"));
    }

    #[test]
    fn subgraph_changes_rejects_unknown_top_level_keys() {
        // `subgraph:` (singular) instead of `subgraphs:` should error rather
        // than silently produce an empty change list.
        let (_fixture, preview) =
            preview_with_changes_file("subgraph:\n  bar:\n    remove: true\n");

        let err = preview.subgraph_changes().unwrap_err();
        assert!(err.to_string().contains("Invalid --subgraph-changes file"));
    }

    #[test]
    fn subgraph_changes_rejects_unknown_entry_fields() {
        let (_fixture, preview) =
            preview_with_changes_file("subgraphs:\n  bar:\n    routing_ur: https://example.com\n");

        let err = preview.subgraph_changes().unwrap_err();
        assert!(err.to_string().contains("Invalid --subgraph-changes file"));
    }
}
