use clap::Parser;
use rover_client::operations::graph_artifact::tag::{
    self, AssignGraphArtifactTagInput, GraphArtifactInput,
};
use serde::Serialize;

use crate::{RoverOutput, RoverResult, options::ProfileOpt, utils::client::StudioClientConfig};

#[derive(Debug, Serialize, Parser)]
pub struct Tag {
    #[clap(flatten)]
    profile: ProfileOpt,
    #[arg(long)]
    graph_id: String,
    tag: String,
    #[clap(flatten)]
    resource_id: ResourceID,
}

#[derive(Debug, Clone, Serialize, Parser)]
#[group(required = true, multiple = false)]
struct ResourceID {
    #[arg(short, long)]
    digest: Option<String>,
    #[arg(short, long)]
    graph_artifact_id: Option<String>,
}

impl Tag {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let input = AssignGraphArtifactTagInput {
            graph_id: self.graph_id.clone(),
            artifact: GraphArtifactInput {
                digest: self.resource_id.digest.clone(),
                id: self.resource_id.graph_artifact_id.clone(),
            },
            tag: self.tag.clone(),
        };

        let assign_tag_response = tag::run(input, &client).await?;
        Ok(RoverOutput::AssignGraphArtifactTagResponse(
            assign_tag_response,
        ))
    }
}
