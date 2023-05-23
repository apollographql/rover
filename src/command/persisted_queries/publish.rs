use clap::Parser;
use rover_std::Style;
use serde::Serialize;

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::FileDescriptorType;
use crate::{RoverOutput, RoverResult};

use rover_client::operations::persisted_queries::publish::{
    self, PersistedQueriesPublishInput, PersistedQueryManifest,
};

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    /// The Graph ID to publish operations to.
    #[serde(skip_serializing)]
    #[arg(long)]
    graph_id: String,

    /// The list ID to publish operations to.
    #[serde(skip_serializing)]
    #[arg(long)]
    list_id: String,

    /// The path to the manifest containing operations to publish.
    #[serde(skip_serializing)]
    #[arg(long)]
    manifest: FileDescriptorType,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Publish {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let raw_manifest = self
            .manifest
            .read_file_descriptor("operation manifest", &mut std::io::stdin())?;

        // FIXME: better error on bad format
        let operation_manifest: PersistedQueryManifest = serde_json::from_str(&raw_manifest)?;

        eprintln!(
            "Publishing operations to list {} for {} using credentials from the {} profile.",
            Style::Link.paint(&self.list_id),
            Style::Link.paint(&self.graph_id),
            Style::Command.paint(&self.profile.profile_name)
        );
        let result = publish::run(
            PersistedQueriesPublishInput {
                graph_id: self.graph_id.clone(),
                list_id: self.list_id.clone(),
                operation_manifest,
            },
            &client,
        )?;
        Ok(RoverOutput::PersistedQueriesPublishResponse(result))
    }
}
