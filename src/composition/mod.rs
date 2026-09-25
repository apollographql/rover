use std::{fmt::Debug, io::stdin};

use anyhow::Error;
use apollo_federation_types::{
    config::{FederationVersion, SchemaSource},
    rover::{BuildErrors, BuildHint},
};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_studio::types::GraphRef;
use tower::ServiceExt;

use crate::{
    RoverError,
    command::install::PluginProvenance,
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
    federation::FederationOneUnsupported,
    options::{LicenseAccepter, PluginOpts, ProfileOpt},
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
#[allow(clippy::too_many_arguments)]
pub(crate) async fn get_supergraph_binary(
    federation_version: Option<FederationVersion>,
    client_config: StudioClientConfig,
    override_install_path: Option<Utf8PathBuf>,
    profile: ProfileOpt,
    plugin_opts: PluginOpts,
    supergraph_yaml: Option<FileDescriptorType>,
    graph_ref: Option<GraphRef>,
    // Only `supergraph compose` nudges users to pin the federation version;
    // `connector` and the LSP share this helper but shouldn't warn.
    warn_on_floating_version: bool,
) -> Result<CompositionPipeline<Run>, RoverError> {
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
            warn_on_floating_version,
        )
        .await?
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
    Binary {
        error: String,
        provenance: Box<PluginProvenance>,
    },
    #[error("The composition binary exited with errors.\nStdout: {}\nStderr: {}", .stdout, .stderr)]
    BinaryExit {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        provenance: Box<PluginProvenance>,
    },
    #[error("Failed to parse output of `{} compose`\n{error}", .provenance.path)]
    InvalidOutput {
        provenance: Box<PluginProvenance>,
        error: String,
    },
    #[error(
        "The composition binary `{} compose` produced no output. This usually means it \
         exited abnormally without composing. Try re-running with `--log debug` for more detail.",
        .provenance.path
    )]
    EmptyOutput { provenance: Box<PluginProvenance> },
    #[error("Invalid input for `{} compose`\n{error}", .provenance.path)]
    InvalidInput {
        provenance: Box<PluginProvenance>,
        error: String,
    },
    #[error("Failed to read the file at: {path}.\n{error}")]
    ReadFile {
        path: Utf8PathBuf,
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to write to the file at: {path}.\n{error}")]
    WriteFile {
        path: Utf8PathBuf,
        error: Box<dyn std::error::Error + Send + Sync>,
        provenance: Option<Box<PluginProvenance>>,
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
        provenance: Box<PluginProvenance>,
    },
    /// Raised beside the binary rather than by it: the temporary directory whose
    /// path Rover builds the resolved supergraph config's filename from, before
    /// handing that path to the plugin.
    ///
    /// Deliberately not `#[from]`. A blanket `From<io::Error>` is what made the
    /// previous version of this mislabel every stray io error as a failure to
    /// run the binary; one explicit call site costs a line and cannot drift.
    #[error("Failed to create a temporary directory for the composition input.")]
    TempDir {
        #[source]
        source: std::io::Error,
        provenance: Option<Box<PluginProvenance>>,
    },
    /// Also raised beside the binary rather than by it: serialising the
    /// resolved config on its way to the plugin. Not `#[from]`, so a `?`
    /// elsewhere cannot silently pick the variant with nowhere to put the
    /// provenance — which is how this one was losing it.
    #[error("Serialization error")]
    SerdeYaml {
        #[source]
        source: serde_yaml::Error,
        provenance: Option<Box<PluginProvenance>>,
    },
    #[error("{}", .0)]
    InvalidSupergraphConfig(String),
    #[error("Error when updating Federation Version")]
    ErrorUpdatingFederationVersion(#[from] InstallSupergraphError),
    #[error("Error resolving subgraphs")]
    ResolvingSubgraphsError {
        #[source]
        source: ResolveSupergraphConfigError,
        provenance: Option<Box<PluginProvenance>>,
    },
    #[error("Could not install supergraph binary")]
    InstallSupergraphBinaryError { source: InstallSupergraphError },
    #[error(transparent)]
    FederationOneUnsupported(#[from] FederationOneUnsupported),
}

impl CompositionError {
    /// The plugin a run had already resolved when this error was raised, if any.
    ///
    /// `Some` for every error the supergraph binary itself produces, so a failed
    /// run can still report under `data` the plugin it used (FR57) — "which
    /// federation version rejected this" being the first question asked of a
    /// composition failure.
    ///
    /// Also `Some` for the errors raised *beside* the binary once it had
    /// resolved — the temp directory, the config write, subgraph resolution. A
    /// run that got that far has already printed the plugin's FR54 line to
    /// stderr, so reporting nothing under `data` would have the same run
    /// answering the same question two different ways.
    ///
    /// `None` only where nothing resolved. That is FR57's one exclusion: a
    /// plugin that never resolved has no source, level or path, and is
    /// identified through the error contract (FR87) instead.
    ///
    /// Listed exhaustively rather than ending in a wildcard. Carrying the
    /// plugin is the contract this type exists to keep, so a variant added
    /// later should not be able to opt out of it silently — the compiler asks
    /// the question instead.
    pub fn provenance(&self) -> Option<&PluginProvenance> {
        match self {
            CompositionError::Binary { provenance, .. }
            | CompositionError::BinaryExit { provenance, .. }
            | CompositionError::InvalidOutput { provenance, .. }
            | CompositionError::EmptyOutput { provenance }
            | CompositionError::InvalidInput { provenance, .. }
            | CompositionError::Build { provenance, .. } => Some(provenance),

            CompositionError::TempDir { provenance, .. }
            | CompositionError::WriteFile { provenance, .. }
            | CompositionError::SerdeYaml { provenance, .. }
            | CompositionError::ResolvingSubgraphsError { provenance, .. } => provenance.as_deref(),

            CompositionError::ReadFile { .. }
            | CompositionError::UpsertFile { .. }
            | CompositionError::InvalidSupergraphConfig(_)
            | CompositionError::ErrorUpdatingFederationVersion(_)
            | CompositionError::InstallSupergraphBinaryError { .. }
            | CompositionError::FederationOneUnsupported(_) => None,
        }
    }
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

#[cfg(test)]
mod tests {
    use rover_std::format_error_chain;
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn serde_yaml_message_excludes_its_cause() {
        let inner = serde_yaml::from_str::<serde_yaml::Value>("[1, 2").unwrap_err();
        let cause = inner.to_string();
        let err = CompositionError::SerdeYaml {
            source: inner,
            provenance: None,
        };

        assert_that!(err.to_string()).is_equal_to("Serialization error".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(format!("Serialization error: {cause}"));
    }

    #[test]
    fn error_updating_federation_version_message_excludes_its_cause() {
        let err = CompositionError::from(InstallSupergraphError::MissingDependency {
            err: "the plugin was not installed".to_string(),
        });

        assert_that!(err.to_string())
            .is_equal_to("Error when updating Federation Version".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(
            "Error when updating Federation Version: unable to find dependency: \"the plugin was not installed\""
                .to_string(),
        );
    }

    #[test]
    fn resolving_subgraphs_error_message_excludes_its_cause() {
        let err = CompositionError::ResolvingSubgraphsError {
            source: ResolveSupergraphConfigError::NoSource,
            provenance: None,
        };

        assert_that!(err.to_string()).is_equal_to("Error resolving subgraphs".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(
            "Error resolving subgraphs: No source found for supergraph config".to_string(),
        );
    }

    #[test]
    fn install_supergraph_binary_error_message_excludes_its_cause() {
        let err = CompositionError::InstallSupergraphBinaryError {
            source: InstallSupergraphError::MissingDependency {
                err: "the plugin was not installed".to_string(),
            },
        };

        assert_that!(err.to_string())
            .is_equal_to("Could not install supergraph binary".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(
            "Could not install supergraph binary: unable to find dependency: \"the plugin was not installed\""
                .to_string(),
        );
    }
}
