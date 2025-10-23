use std::{fmt::Debug, io::stdin};

use anyhow::Error;
use apollo_federation_types::{
    config::{FederationVersion, SchemaSource},
    rover::{BuildErrors, BuildHint},
};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use tower::ServiceExt;

use crate::{
    RoverError,
    composition::{
        pipeline::{CompositionPipeline, state::Run},
        supergraph::{
            config::{
                error::ResolveSubgraphError,
                full::introspect::MakeResolveIntrospectSubgraph,
                resolver::{
                    LoadRemoteSubgraphsError, LoadSupergraphConfigError,
                    ResolveSupergraphConfigError, fetch_remote_subgraph::MakeFetchRemoteSubgraph,
                    fetch_remote_subgraphs::MakeFetchRemoteSubgraphs,
                },
            },
            install::InstallSupergraphError,
        },
    },
    options::{LicenseAccepter, PluginOpts},
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
};

pub mod events;
pub mod pipeline;
pub mod runner;
pub mod supergraph;
#[cfg(test)]
pub mod test;
pub mod types;

#[cfg(feature = "composition-js")]
mod watchers;

/// A reusable, shareable, canonical way to get a supergraph binary from the common options
/// used around Rover.
pub(crate) async fn get_supergraph_binary(
    federation_version: Option<FederationVersion>,
    client_config: StudioClientConfig,
    override_install_path: Option<Utf8PathBuf>,
    plugin_opts: PluginOpts,
    supergraph_yaml: Option<FileDescriptorType>,
    graph_ref: Option<GraphRef>,
) -> Result<CompositionPipeline<Run>, RoverError> {
    let profile = plugin_opts.profile;

    let fetch_remote_subgraphs_factory = MakeFetchRemoteSubgraphs::builder()
        .studio_client_config(client_config.clone())
        .profile(profile.clone())
        .build();

    let fetch_remote_subgraph_factory = MakeFetchRemoteSubgraph::builder()
        .studio_client_config(client_config.clone())
        .profile(profile.clone())
        .build()
        .boxed_clone();
    let resolve_introspect_subgraph_factory =
        MakeResolveIntrospectSubgraph::new(client_config.service()?).boxed_clone();

    CompositionPipeline::default()
        .init(
            &mut stdin(),
            fetch_remote_subgraphs_factory,
            supergraph_yaml,
            graph_ref.clone(),
            None,
        )
        .await?
        .resolve_federation_version(
            resolve_introspect_subgraph_factory,
            fetch_remote_subgraph_factory,
            federation_version,
        )
        .await
        .install_supergraph_binary(
            client_config,
            override_install_path,
            plugin_opts.elv2_license_accepter,
            plugin_opts.skip_update,
        )
        .await
        .map_err(RoverError::from)
}

#[derive(Debug, Clone)]
pub struct FederationUpdaterConfig {
    pub(crate) studio_client_config: StudioClientConfig,
    pub(crate) elv2_licence_accepter: LicenseAccepter,
    pub(crate) skip_update: bool,
}

#[derive(Getters, Debug, Clone, Eq, PartialEq)]
pub struct CompositionSuccess {
    pub(crate) supergraph_sdl: String,
    pub(crate) hints: Vec<BuildHint>,
    pub(crate) federation_version: FederationVersion,
}

#[derive(thiserror::Error, Debug)]
pub enum CompositionError {
    #[error("Failed to run the composition binary")]
    Binary { error: String },
    #[error("The composition binary exited with errors.\nStdout: {}\nStderr: {}", .stdout, .stderr)]
    BinaryExit {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("Failed to parse output of `{binary} compose`\n{error}")]
    InvalidOutput { binary: Utf8PathBuf, error: String },
    #[error("Invalid input for `{binary} compose`\n{error}")]
    InvalidInput { binary: Utf8PathBuf, error: String },
    #[error("Failed to read the file at: {path}.\n{error}")]
    ReadFile {
        path: Utf8PathBuf,
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to write to the file at: {path}.\n{error}")]
    WriteFile {
        path: Utf8PathBuf,
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to upsert the file at: {path}.\n{error}")]
    UpsertFile {
        path: Utf8PathBuf,
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Encountered {} while trying to build a supergraph.", .source.length_string())]
    Build {
        source: BuildErrors,
        federation_version: FederationVersion,
    },
    #[error("Serialization error.\n{}", .0)]
    SerdeYaml(#[from] serde_yaml::Error),
    #[error("{}", .0)]
    InvalidSupergraphConfig(String),
    #[error("Error when updating Federation Version:\n{}", .0)]
    ErrorUpdatingFederationVersion(#[from] InstallSupergraphError),
    #[error("Error resolving subgraphs:\n{}", .0)]
    ResolvingSubgraphsError(#[from] ResolveSupergraphConfigError),
    #[error("Could not install supergraph binary:\n{}", .source)]
    InstallSupergraphBinaryError { source: InstallSupergraphError },
}

#[derive(Debug, Eq, PartialEq)]
pub struct CompositionSubgraphAdded {
    pub(crate) name: String,
    pub(crate) schema_source: SchemaSource,
}

#[derive(Debug)]
pub struct CompositionSubgraphRemoved {
    pub(crate) name: String,
    pub(crate) resolution_error: Option<ResolveSubgraphError>,
}

#[derive(thiserror::Error, Debug)]
pub enum SupergraphConfigResolutionError {
    #[error("Could not instantiate Studio Client")]
    StudioClientInitialisationFailed(#[from] Error),
    #[error("Could not load remote subgraphs")]
    LoadRemoteSubgraphsFailed(#[from] LoadRemoteSubgraphsError),
    #[error("Could not load supergraph config from local file.\n{}", .0)]
    LoadLocalSupergraphConfigFailed(#[from] LoadSupergraphConfigError),
    #[error("Could not resolve local and remote elements into complete SupergraphConfig")]
    ResolveSupergraphConfigFailed(#[from] ResolveSupergraphConfigError),
}
