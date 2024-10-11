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
    graph_ref: String,
    subgraph: String,
    profile: ProfileOpt,
    client_config: StudioClientConfig,
}

impl RemoteSchema {
    pub fn new(
        graph_ref: String,
        subgraph: String,
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
    ) -> Self {
        Self {
            graph_ref,
            subgraph,
            profile: profile.clone(),
            client_config: client_config.clone(),
        }
    }

    pub async fn run(&self) -> Result<String, RoverError> {
        let client = self.client_config.get_authenticated_client(&self.profile)?;

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
