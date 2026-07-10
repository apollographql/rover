use clap::Parser;
use rover_client::operations::graph_artifact::fetch::{
    self, FetchGraphArtifactInput, GraphArtifactIdentifier,
};
use serde::Serialize;

use crate::{RoverOutput, RoverResult, options::ProfileOpt, utils::client::StudioClientConfig};

#[derive(Debug, Serialize, Parser)]
pub struct Fetch {
    #[clap(flatten)]
    profile: ProfileOpt,
    #[arg(long)]
    graph_id: String,
    #[clap(flatten)]
    identifier: Identifier,
    /// The number of history entries to show when fetching by tag (max 20).
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..=20))]
    history_limit: u32,
}

#[derive(Debug, Clone, Serialize, Parser)]
#[group(required = true, multiple = false)]
struct Identifier {
    #[arg(short, long)]
    tag_name: Option<String>,
    #[arg(short, long)]
    graph_artifact_id: Option<String>,
    #[arg(short, long)]
    digest: Option<String>,
}

impl Fetch {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        // The argument group guarantees exactly one of these is set.
        let identifier = if let Some(tag) = &self.identifier.tag_name {
            GraphArtifactIdentifier::Tag(tag.clone())
        } else if let Some(id) = &self.identifier.graph_artifact_id {
            GraphArtifactIdentifier::Id(id.clone())
        } else if let Some(digest) = &self.identifier.digest {
            GraphArtifactIdentifier::Digest(digest.clone())
        } else {
            unreachable!("clap argument group guarantees one identifier is provided");
        };

        let input = FetchGraphArtifactInput {
            graph_id: self.graph_id.clone(),
            identifier,
            history_limit: self.history_limit.into(),
        };

        let response = fetch::run(input, &client).await?;
        Ok(RoverOutput::FetchGraphArtifactResponse(response))
    }
}
