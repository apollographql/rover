use crate::RoverResult;
use crate::composition::supergraph::config::unresolved::UnresolvedSubgraph;
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use rover_client::blocking::StudioClient;
use rover_client::operations::init::build_pipeline_track;
use std::collections::BTreeMap;
use std::fs::read_to_string;
use thiserror::Error;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use rover_client::operations::init::build_pipeline_track::*;
use rover_client::operations::subgraph::publish::*;
use rover_client::shared::GitContext;
use rover_client::shared::GraphRef;
use semver::Version;

#[derive(Debug, Error, Clone)]
pub enum GraphOperationError {
    #[error("Failed to authenticate with GraphOS")]
    AuthenticationFailed,
    #[error("Failed to create API key: {0}")]
    KeyCreationFailed(String),
    #[error("Failed to parse federation version: {0}")]
    FederationVersionParseError(String),
}

//This maps the federation version we pull from templates in GitHub to the build pipeline track.
fn map_federation_version_to_build_pipeline_track(
    version_str: &str,
) -> Result<BuildPipelineTrack, GraphOperationError> {
    let clean_version = version_str.trim_start_matches(['=', 'v', 'V']);

    let complete_version = if clean_version.matches('.').count() == 1 {
        format!("{clean_version}.0")
    } else {
        clean_version.to_string()
    };

    let version = Version::parse(&complete_version).map_err(|e| {
        GraphOperationError::FederationVersionParseError(format!(
            "Failed to parse version '{complete_version}': {e}"
        ))
    })?;

    match (version.major, version.minor) {
        (1, 0) => Ok(BuildPipelineTrack::FED_1_0),
        (1, 1) => Ok(BuildPipelineTrack::FED_1_1),
        (2, 0) => Ok(BuildPipelineTrack::FED_2_0),
        (2, 1) => Ok(BuildPipelineTrack::FED_2_1),
        (2, 3) => Ok(BuildPipelineTrack::FED_2_3),
        (2, 4) => Ok(BuildPipelineTrack::FED_2_4),
        (2, 5) => Ok(BuildPipelineTrack::FED_2_5),
        (2, 6) => Ok(BuildPipelineTrack::FED_2_6),
        (2, 7) => Ok(BuildPipelineTrack::FED_2_7),
        (2, 8) => Ok(BuildPipelineTrack::FED_2_8),
        (2, 9) => Ok(BuildPipelineTrack::FED_2_9),
        (2, 10) => Ok(BuildPipelineTrack::FED_2_10),
        (2, 11) => Ok(BuildPipelineTrack::FED_2_11),
        _ => Err(GraphOperationError::FederationVersionParseError(format!(
            "Unsupported federation version: {version}"
        ))),
    }
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
    graph_ref: &GraphRef,
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
        let schema_path = UnresolvedSubgraph::resolve_file_path(
            subgraph_name,
            output_path,
            &schema_path.unwrap(),
        )?;
        let sdl = read_to_string(schema_path)?;
        rover_client::operations::subgraph::publish::run(
            SubgraphPublishInput {
                graph_ref: graph_ref.clone(),
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

pub(crate) async fn update_variant_federation_version(
    client: &StudioClient,
    graph_ref: &GraphRef,
    federation_version: Option<String>,
) -> RoverResult<BuildPipelineTrackResponse> {
    let build_pipeline_track_input = BuildPipelineTrackInput {
        graph_id: graph_ref.name.clone(),
        variant_name: graph_ref.variant.clone(),
        version: federation_version
            .map(|v| map_federation_version_to_build_pipeline_track(&v))
            .transpose()?
            .unwrap(),
    };

    let build_pipeline_track_response =
        build_pipeline_track::run(build_pipeline_track_input, client).await?;
    Ok(build_pipeline_track_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rover_client::operations::init::build_pipeline_track::BuildPipelineTrack;

    #[test]
    fn test_map_federation_version_to_build_pipeline_track_valid_versions() {
        assert_eq!(
            map_federation_version_to_build_pipeline_track("1.0").unwrap(),
            BuildPipelineTrack::FED_1_0
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("1.1").unwrap(),
            BuildPipelineTrack::FED_1_1
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.0").unwrap(),
            BuildPipelineTrack::FED_2_0
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.1").unwrap(),
            BuildPipelineTrack::FED_2_1
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.3").unwrap(),
            BuildPipelineTrack::FED_2_3
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.4").unwrap(),
            BuildPipelineTrack::FED_2_4
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.5").unwrap(),
            BuildPipelineTrack::FED_2_5
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.6").unwrap(),
            BuildPipelineTrack::FED_2_6
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.7").unwrap(),
            BuildPipelineTrack::FED_2_7
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.8").unwrap(),
            BuildPipelineTrack::FED_2_8
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.9").unwrap(),
            BuildPipelineTrack::FED_2_9
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.10").unwrap(),
            BuildPipelineTrack::FED_2_10
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("2.11").unwrap(),
            BuildPipelineTrack::FED_2_11
        );
    }

    #[test]
    fn test_map_federation_version_to_build_pipeline_track_with_prefixes() {
        assert_eq!(
            map_federation_version_to_build_pipeline_track("v2.0").unwrap(),
            BuildPipelineTrack::FED_2_0
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("=2.0").unwrap(),
            BuildPipelineTrack::FED_2_0
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("v2.11").unwrap(),
            BuildPipelineTrack::FED_2_11
        );
        assert_eq!(
            map_federation_version_to_build_pipeline_track("=2.11").unwrap(),
            BuildPipelineTrack::FED_2_11
        );
    }

    #[test]
    fn test_map_federation_version_to_build_pipeline_track_invalid_versions() {
        assert!(map_federation_version_to_build_pipeline_track("invalid").is_err());
        assert!(map_federation_version_to_build_pipeline_track("2.11.2.preview").is_err());
        assert!(map_federation_version_to_build_pipeline_track("2.").is_err());
        assert!(map_federation_version_to_build_pipeline_track(".0").is_err());
    }
}
