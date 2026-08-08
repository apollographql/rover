use anyhow::anyhow;
use clap::{ArgGroup, Parser};
use rover_client::operations::contract::preview::{
    self, ContractFilterConfig, ContractPreviewInput, ContractPreviewStatusInput,
};
use rover_std::Style;
use serde::Serialize;

use crate::{
    RoverError, RoverOutput, RoverResult,
    options::{GraphRefOpt, ProfileOpt},
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
    /// Required to identify graph and check permissions even though buildId is unique.
    #[clap(flatten)]
    graph: GraphRefOpt,

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
    /// flag, Rover starts the build and polls until it finishes. With this
    /// flag, Rover only starts the build (check on it later with --build-id).
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
        ]
    )]
    build_id: Option<String>,
}

impl Preview {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        checks_timeout_seconds: u64,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.clone();

        if let Some(build_id) = &self.build_id {
            eprintln!(
                "Checking status of contract preview job {} on {} using credentials from the {} profile.",
                Style::Link.paint(build_id),
                Style::Link.paint(graph_ref.to_string()),
                Style::Command.paint(&self.profile.profile_name)
            );
            let preview_response = preview::result(
                ContractPreviewStatusInput {
                    graph_ref,
                    build_id: build_id.clone(),
                },
                &client,
            )
            .await?;
            return Ok(RoverOutput::PreviewJob(preview_response));
        }

        eprintln!(
            "Previewing contract schema for {} using credentials from the {} profile.",
            Style::Link.paint(graph_ref.to_string()),
            Style::Command.paint(&self.profile.profile_name)
        );

        // contractPreviewAsync: filter the variant's already-composed
        // supergraph. Filtering is mandatory here.
        let input = ContractPreviewInput {
            graph_ref: graph_ref.clone(),
            filter_config: self.required_filter_config()?,
        };
        let started = preview::start(input, &client).await?;

        if self.asynchronous {
            return Ok(RoverOutput::PreviewJob(started));
        }

        eprintln!(
            "Waiting for the build to complete... or press Ctrl+C and check later with {}.",
            Style::Command.paint(format!(
                "`rover contract preview {} --build-id {}`",
                graph_ref, started.build_id
            ))
        );

        let preview_response = preview::poll(
            ContractPreviewStatusInput {
                graph_ref,
                build_id: started.build_id,
            },
            &client,
            checks_timeout_seconds,
        )
        .await?;

        Ok(RoverOutput::PreviewJob(preview_response))
    }

    /// Builds the filter config from the paired include/exclude/hide flags,
    /// enforcing that exactly one flag from each pair was provided.
    ///
    /// This is enforced at runtime rather than with `ArgGroup::required(true)`
    /// (as `contract publish` does) because the pairs are not required in
    /// `--build-id` mode.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A `Preview` with every include/exclude/hide flag explicitly decided,
    /// except `--build-id` which changes to polling. Individual tests override
    /// one field at a time to exercise each validation branch.
    fn valid_preview() -> Preview {
        Preview {
            graph: GraphRefOpt {
                graph_ref: "test-graph@current".parse().unwrap(),
            },
            profile: ProfileOpt::default(),
            include_tag: vec!["foo".to_string()],
            no_include_tags: false,
            exclude_tag: Vec::new(),
            no_exclude_tags: true,
            hide_unreachable_types: true,
            no_hide_unreachable_types: false,
            asynchronous: false,
            build_id: None,
        }
    }

    #[test]
    fn required_filter_config_builds_config_when_all_three_pairs_are_decided() {
        let config = valid_preview().required_filter_config().unwrap();
        assert_eq!(config.include, vec!["foo".to_string()]);
        assert_eq!(config.exclude, Vec::<String>::new());
        assert!(config.hide_unreachable_types);
    }

    #[test]
    fn required_filter_config_allows_no_include_tags_for_an_empty_include_list() {
        let preview = Preview {
            include_tag: Vec::new(),
            no_include_tags: true,
            ..valid_preview()
        };
        let config = preview.required_filter_config().unwrap();
        assert_eq!(config.include, Vec::<String>::new());
    }

    #[test]
    fn required_filter_config_errors_when_include_tags_are_undecided() {
        let preview = Preview {
            include_tag: Vec::new(),
            no_include_tags: false,
            ..valid_preview()
        };
        let err = preview.required_filter_config().unwrap_err();
        assert!(err.to_string().contains("--include-tag"));
    }

    #[test]
    fn required_filter_config_errors_when_exclude_tags_are_undecided() {
        let preview = Preview {
            exclude_tag: Vec::new(),
            no_exclude_tags: false,
            ..valid_preview()
        };
        let err = preview.required_filter_config().unwrap_err();
        assert!(err.to_string().contains("--exclude-tag"));
    }

    #[test]
    fn required_filter_config_errors_when_hide_unreachable_types_is_undecided() {
        let preview = Preview {
            hide_unreachable_types: false,
            no_hide_unreachable_types: false,
            ..valid_preview()
        };
        let err = preview.required_filter_config().unwrap_err();
        assert!(err.to_string().contains("--hide-unreachable-types"));
    }
}
