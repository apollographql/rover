use anyhow::Context;
use clap::Parser;
use rover_client::operations::persisted_queries::publish::{
    self, ApolloPersistedQueryManifest, PersistedQueriesPublishInput, RelayPersistedQueryManifest,
};
use rover_std::Style;
use serde::Serialize;

use super::identify::identify_persisted_query_list;
use crate::{
    RoverOutput, RoverResult,
    options::{OptionalGraphRefOpt, PersistedQueriesManifestFormat, ProfileOpt},
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
};

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

    /// The format of the manifest file.
    #[arg(long, value_enum, default_value_t = PersistedQueriesManifestFormat::Apollo)]
    manifest_format: PersistedQueriesManifestFormat,

    /// If provided, overrides the `clientName` field in all operations in
    /// the manifest file.
    #[arg(long)]
    for_client_name: Option<String>,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Publish {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let raw_manifest = self
            .manifest
            .read_file_descriptor("operation manifest", &mut std::io::stdin())?;

        let invalid_json_err = |manifest, format| {
            format!("JSON in {manifest} did not match '--manifest-format {format}'")
        };

        let mut operation_manifest = match self.manifest_format {
            PersistedQueriesManifestFormat::Apollo => {
                serde_json::from_str::<ApolloPersistedQueryManifest>(&raw_manifest)
                    .with_context(|| invalid_json_err(&self.manifest, "apollo"))?
            }
            PersistedQueriesManifestFormat::Relay => {
                serde_json::from_str::<RelayPersistedQueryManifest>(&raw_manifest)
                    .with_context(|| invalid_json_err(&self.manifest, "relay"))?
                    .try_into()?
            }
        };

        // Override any client names provided in the manifest (which is the only way to
        // provide client names for the Relay format).
        if let Some(for_client_name) = &self.for_client_name {
            for op in &mut operation_manifest.operations {
                op.client_name = Some(for_client_name.to_string());
            }
        }

        let (graph_id, list_id, list_name) = identify_persisted_query_list(
            &self.graph,
            &self.graph_id,
            &self.list_id,
            "publishing operations to",
            &client,
        )
        .await?;

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
        )
        .await?;
        Ok(RoverOutput::PersistedQueriesPublishResponse(result))
    }
}
