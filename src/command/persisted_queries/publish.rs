use anyhow::{anyhow, Context};
use clap::Parser;
use rover_client::operations::persisted_queries::name::{self, PersistedQueryListNameInput};
use rover_std::{Emoji, Style};
use serde::Serialize;

use crate::options::{OptionalGraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::FileDescriptorType;
use crate::{RoverOutput, RoverResult};

use rover_client::operations::persisted_queries::publish::{
    self, PersistedQueriesPublishInput, PersistedQueryManifest,
};
use rover_client::operations::persisted_queries::resolve::{self, ResolvePersistedQueryListInput};

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    #[clap(flatten)]
    graph: OptionalGraphRefOpt,

    /// The Graph ID to publish operations to.
    #[serde(skip_serializing)]
    #[arg(long, conflicts_with = "graph_ref")]
    graph_id: Option<String>,

    /// The list ID to publish operations to.
    #[serde(skip_serializing)]
    #[arg(long, conflicts_with = "graph_ref")]
    list_id: Option<String>,

    /// The path to the manifest containing operations to publish.
    #[serde(skip_serializing)]
    #[arg(long)]
    manifest: FileDescriptorType,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Publish {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        eprintln!("{} This feature is currently in preview, this feature must be enabled for your GraphOS account for it to work.", Emoji::Warn);
        let client = client_config.get_authenticated_client(&self.profile)?;

        let raw_manifest = self
            .manifest
            .read_file_descriptor("operation manifest", &mut std::io::stdin())?;

        let operation_manifest: PersistedQueryManifest = serde_json::from_str(&raw_manifest)
            .with_context(|| format!("JSON in {raw_manifest} was invalid"))?;

        let (graph_id, list_id) = match (&self.graph.graph_ref, &self.graph_id, &self.list_id) {
            (Some(graph_ref), None, None) => {
                let persisted_query_list = resolve::run(ResolvePersistedQueryListInput { graph_ref: graph_ref.clone() }, &client)?;
                (graph_ref.clone().name, persisted_query_list.id)
            },
            (None, Some(graph_id), Some(list_id)) => {
                (graph_id.to_string(), list_id.to_string())
            },
            (None, Some(graph_id), None) => {
                return Err(anyhow!("You must specify a --list-id <LIST_ID> when publishing operations to --graph-id {graph_id}, or, if a list is linked to a specific variant, you can leave --graph-id unspecified, and pass a full graph ref as a positional argument.").into())
            }
            (None, None, Some(list_id)) => {
                return Err(anyhow!("You must specify a --graph-id <GRAPH_ID> when publishing operations to --list-id {list_id}, or, if {list_id} is linked to a specific variant, you can leave --list-id unspecified, and pass a full graph ref as a positional argument.").into())
            }
            (None, None, None) => {
                return Err(anyhow!("You must either specify a <GRAPH_REF> that has a linked persisted query list OR both a --graph_id <GRAPH_ID> and --list_id <LIST_ID>").into())
            },
            (Some(_), Some(_), Some(_)) | (Some(_), Some(_), None) | (Some(_), None, Some(_)) => unreachable!("clap \"conflicts_with\" should make this impossible to reach")
        };

        let list_name = name::run(
            PersistedQueryListNameInput {
                graph_id: graph_id.clone(),
                list_id: list_id.clone(),
            },
            &client,
        )?
        .name;

        eprintln!(
            "Publishing operations to list {} for {} using credentials from the {} profile.",
            Style::Link.paint(list_name),
            Style::Link.paint(&graph_id),
            Style::Command.paint(&self.profile.profile_name)
        );

        let result = publish::run(
            PersistedQueriesPublishInput {
                graph_id,
                list_id,
                operation_manifest,
            },
            &client,
        )?;
        Ok(RoverOutput::PersistedQueriesPublishResponse(result))
    }
}
