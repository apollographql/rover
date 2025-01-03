use crate::composition::supergraph::config::full::FullyResolveSubgraph;
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::RoverError;

use rover_client::operations::subgraph::fetch;
use rover_client::operations::subgraph::fetch::SubgraphFetchInput;
use rover_client::shared::GraphRef;

use std::str::FromStr;

/// Remote schemas are fetched from Studio and are a GraphRef and subgraph name combination
#[derive(Debug, Clone)]
pub struct RemoteSchema {
    resolver: FullyResolveSubgraph,
}

impl RemoteSchema {
    pub fn new(resolver: FullyResolveSubgraph) -> Self {
        Self { resolver }
    }

    pub async fn run(&self) -> Result<String, Resolve> {
        let service = self.service.clone().ready().await?;

        let response = fetch::run(
            SubgraphFetchInput {
                graph_ref: GraphRef::from_str(&self.graph_ref)?,
                subgraph_name: self.subgraph.clone(),
            },
            &client,
        )
        .await
        .map_err(RoverError::from)?;

        Ok(response.sdl.contents)
    }
}
