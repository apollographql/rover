use clap::{ArgGroup, Parser};
use serde::Serialize;

use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::operations::contract::publish::{self, ContractPublishInput};
use rover_client::RoverClientError;
use rover_std::Style;

#[derive(Debug, Serialize, Parser)]
#[clap(
    group = ArgGroup::new("include_group")
        .args(&["include", "no_include"]).required(true).multiple(false),
    group = ArgGroup::new("exclude_group")
        .args(&["exclude", "no_exclude"]).required(true).multiple(false),
    group = ArgGroup::new("hide_unreachable_types_group")
        .args(&["hide_unreachable_types", "no_hide_unreachable_types"]).required(true).multiple(false)
)]
pub struct Publish {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// The source variant name for this contract variant. Once set, this cannot be changed.
    #[arg(long)]
    #[serde(skip_serializing)]
    source_variant: Option<String>,

    /// List of tag names to include in the contract schema (e.g. '--include foo --include bar').
    /// To specify an empty list, use --no-include instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    include: Vec<String>,

    /// Use an empty include list of tag names for the contract schema.
    /// To specify a non-empty list, use --include instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    no_include: bool,

    /// List of tag names to exclude from the contract schema (e.g. '--exclude foo --exclude bar').
    /// To specify an empty list, use --no-exclude instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    exclude: Vec<String>,

    /// Use an empty exclude list of tag names for the contract schema.
    /// To specify a non-empty list, use --exclude instead.
    #[arg(long)]
    #[serde(skip_serializing)]
    no_exclude: bool,

    /// Automatically hide types that can never be reached in operations on the contract schema.
    #[arg(long)]
    #[serde(skip_serializing)]
    hide_unreachable_types: bool,

    /// Do not automatically hide types that can never be reached in operations on the contract schema.
    #[arg(long)]
    #[serde(skip_serializing)]
    no_hide_unreachable_types: bool,

    /// Do not trigger a launch in Studio after updating the contract configuration.
    #[arg(long)]
    no_launch: bool,
}

impl Publish {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        eprintln!(
            "Publishing configuration to {} using credentials from the {} profile.\n",
            Style::Link.paint(&self.graph.graph_ref.to_string()),
            Style::Command.paint(&self.profile.profile_name)
        );

        let include = if !self.include.is_empty() {
            Some(self.include.clone())
        } else if self.no_include {
            Some(Vec::new())
        } else {
            None
        };
        let exclude = if !self.exclude.is_empty() {
            Some(self.exclude.clone())
        } else if self.no_exclude {
            Some(Vec::new())
        } else {
            None
        };
        let hide_unreachable_types = if self.hide_unreachable_types {
            Some(true)
        } else if self.no_hide_unreachable_types {
            Some(false)
        } else {
            None
        };

        let publish_response = publish::run(
            ContractPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                source_variant: self.source_variant.clone(),
                include: include.ok_or(RoverClientError::AdhocError {
                    msg: "include list unexpectedly absent".to_string(),
                })?,
                exclude: exclude.ok_or(RoverClientError::AdhocError {
                    msg: "exclude list unexpectedly absent".to_string(),
                })?,
                hide_unreachable_types: hide_unreachable_types.ok_or(
                    RoverClientError::AdhocError {
                        msg: "hide_unreachable_types unexpectedly absent".to_string(),
                    },
                )?,
                no_launch: self.no_launch,
            },
            &client,
        )?;

        Ok(RoverOutput::ContractPublish(publish_response))
    }
}
