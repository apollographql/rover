use std::{collections::BTreeMap, fs::read_to_string};

use anyhow::anyhow;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use camino::Utf8PathBuf;
use rover_client::{
    blocking::StudioClient,
    operations::{
        init::{build_pipeline_track, build_pipeline_track::*},
        subgraph::publish::*,
    },
    shared::GitContext,
};
use rover_studio::types::GraphRef;
use semver::Version;
use thiserror::Error;

use crate::{
    RoverResult, composition::supergraph::config::unresolved::UnresolvedSubgraph,
    federation::FederationOneUnsupported, options::ProfileOpt, utils::client::StudioClientConfig,
};

#[derive(Debug, Error, Clone)]
pub enum GraphOperationError {
    #[error("Failed to authenticate with GraphOS")]
    AuthenticationFailed,
    #[error("Failed to create API key: {0}")]
    KeyCreationFailed(String),
    #[error("Failed to parse federation version: {0}")]
    FederationVersionParseError(String),
    #[error(transparent)]
    FederationOneUnsupported(#[from] FederationOneUnsupported),
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
        (1, _) => Err(FederationOneUnsupported.into()),
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
                changelog_message: None,
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
        graph_id: graph_ref.graph_id().to_string(),
        variant_name: graph_ref.variant().to_string(),
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
    use rover_client::operations::init::build_pipeline_track::BuildPipelineTrack;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case("2.0", BuildPipelineTrack::FED_2_0)]
    #[case("2.1", BuildPipelineTrack::FED_2_1)]
    #[case("2.3", BuildPipelineTrack::FED_2_3)]
    #[case("2.4", BuildPipelineTrack::FED_2_4)]
    #[case("2.5", BuildPipelineTrack::FED_2_5)]
    #[case("2.6", BuildPipelineTrack::FED_2_6)]
    #[case("2.7", BuildPipelineTrack::FED_2_7)]
    #[case("2.8", BuildPipelineTrack::FED_2_8)]
    #[case("2.9", BuildPipelineTrack::FED_2_9)]
    #[case("2.10", BuildPipelineTrack::FED_2_10)]
    #[case("2.11", BuildPipelineTrack::FED_2_11)]
    #[case("v2.0", BuildPipelineTrack::FED_2_0)]
    #[case("=2.0", BuildPipelineTrack::FED_2_0)]
    #[case("v2.11", BuildPipelineTrack::FED_2_11)]
    #[case("=2.11", BuildPipelineTrack::FED_2_11)]
    fn test_map_federation_version_to_build_pipeline_track_with_prefixes(
        #[case] input: &str,
        #[case] expectation: BuildPipelineTrack,
    ) {
        assert_that!(map_federation_version_to_build_pipeline_track(input))
            .is_ok()
            .is_equal_to(expectation);
    }

    #[rstest]
    #[case("invalid")]
    #[case("2.11.2.preview")]
    #[case("2.")]
    #[case(".0")]
    fn test_map_federation_version_to_build_pipeline_track_invalid_versions(#[case] input: &str) {
        assert_that(&map_federation_version_to_build_pipeline_track(input))
            .is_err()
            .matches(|err| matches!(err, GraphOperationError::FederationVersionParseError(_)));
    }

    #[rstest]
    #[case::one_zero("1.0")]
    #[case::one_one("1.1")]
    fn test_map_federation_version_to_build_pipeline_track_rejects_federation_one(
        #[case] version: &str,
    ) {
        let err = map_federation_version_to_build_pipeline_track(version).unwrap_err();
        assert_that!(matches!(
            err,
            GraphOperationError::FederationOneUnsupported(_)
        ))
        .is_true();
    }
}
