use clap::Parser;
use rover_client::operations::graph_artifact::list_tags::{self, ListTagsInput};
use serde::Serialize;

use super::list_tags_output::ListTagsOutput;
use crate::{RoverOutput, RoverResult, options::ProfileOpt, utils::client::StudioClientConfig};

#[derive(Debug, Serialize, Parser)]
pub struct ListTags {
    #[clap(flatten)]
    profile: ProfileOpt,
    #[arg(long)]
    graph_id: String,
    #[arg(long)]
    digest: Option<String>,
    /// Maximum number of tags to return. Listing stops once this many tags have
    /// been collected, so listing against a graph with a very large number of
    /// tags doesn't have to paginate through every one.
    #[arg(long, default_value_t = 100)]
    limit: usize,
}

impl ListTags {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let input = match &self.digest {
            Some(digest) => ListTagsInput::ByDigest {
                graph_id: self.graph_id.clone(),
                digest: digest.clone(),
            },
            None => ListTagsInput::ByGraph {
                graph_id: self.graph_id.clone(),
            },
        };

        let response = list_tags::run(input, self.limit, &client).await?;
        Ok(RoverOutput::CliOutput(Box::new(ListTagsOutput(response))))
    }
}
