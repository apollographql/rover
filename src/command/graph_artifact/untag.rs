use clap::Parser;
use rover_client::operations::graph_artifact::untag::{self, DeleteGraphArtifactTagInput};
use serde::Serialize;

use crate::{RoverOutput, RoverResult, options::ProfileOpt, utils::client::StudioClientConfig};

#[derive(Debug, Serialize, Parser)]
pub struct Untag {
    #[clap(flatten)]
    profile: ProfileOpt,
    #[arg(long)]
    graph_id: String,
    tag: String,
}

impl Untag {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let input = DeleteGraphArtifactTagInput {
            graph_id: self.graph_id.clone(),
            tag: self.tag.clone(),
        };

        let delete_tag_response = untag::run(input, &client).await?;
        Ok(RoverOutput::DeleteGraphArtifactTagResponse(
            delete_tag_response,
        ))
    }
}
