use crate::composition::supergraph::config::unresolved::UnresolvedSubgraph;
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::RoverResult;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use rover_client::blocking::StudioClient;
use std::collections::BTreeMap;
use std::fs::read_to_string;
use thiserror::Error;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use rover_client::operations::subgraph::publish::*;
use rover_client::shared::GitContext;
use rover_client::shared::GraphRef;

const DEFAULT_VARIANT: &str = "current";

#[derive(Debug, Error)]
pub enum GraphOperationError {
    #[error("Failed to authenticate with GraphOS")]
    AuthenticationFailed,
    #[error("Failed to create API key: {0}")]
    KeyCreationFailed(String),
}

pub(crate) async fn create_api_key(
    client_config: &StudioClientConfig,
    profile: &ProfileOpt,
    graph_id: String,
    key_name: String,
) -> RoverResult<String> {
    let client = client_config
        .get_authenticated_client(profile)
        .map_err(|_| GraphOperationError::AuthenticationFailed)?;

    let key_input = rover_client::operations::init::key::InitNewKeyInput {
        graph_id,
        key_name,
        role: rover_client::operations::init::key::UserPermission::GraphAdmin,
    };

    let key_response = rover_client::operations::init::key::run(key_input, &client)
        .await
        .map_err(|e| GraphOperationError::KeyCreationFailed(e.to_string()))?;

    Ok(key_response.token)
}

pub(crate) async fn publish_subgraphs(
    client: &StudioClient,
    output_path: &Utf8PathBuf,
    graph_id: String,
    subgraphs: BTreeMap<String, SubgraphConfig>,
) -> RoverResult<()> {
    for (subgraph_name, subgraph_config) in subgraphs.iter() {
        let schema_path = match &subgraph_config.schema {
            SchemaSource::File { file } => Utf8PathBuf::from_path_buf(file.to_path_buf()),
            _ => {
                return Err(
                    anyhow!("Unsupported schema source for subgraph: {}", subgraph_name).into(),
                );
            }
        };
        let unresolved = UnresolvedSubgraph::new(subgraph_name.clone(), subgraph_config.clone());
        let schema_path = unresolved.resolve_file_path(output_path, &schema_path.unwrap())?;
        let sdl = read_to_string(schema_path)?;
        rover_client::operations::subgraph::publish::run(
            SubgraphPublishInput {
                graph_ref: GraphRef {
                    name: graph_id.clone(),
                    variant: DEFAULT_VARIANT.to_string(),
                },
                subgraph: subgraph_name.to_string(),
                url: subgraph_config.routing_url.clone(),
                schema: sdl,
                git_context: GitContext {
                    branch: None,
                    commit: None,
                    author: None,
                    remote_url: None,
                },
                convert_to_federated_graph: false,
            },
            client,
        )
        .await?;
    }
    Ok(())
}
