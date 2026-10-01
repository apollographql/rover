use std::{
    collections::{BTreeMap, HashMap},
    env::current_dir,
    fmt::Debug,
    fs::canonicalize,
};

use apollo_federation_types::config::{
    FederationVersion, FederationVersion::LatestFedTwo, SubgraphConfig,
};
use camino::Utf8PathBuf;
use rover_http::HttpService;
use rover_std::{Style, warnln};
use rover_studio::types::GraphRef;
use tempfile::tempdir;
use tower::MakeService;
use tracing::{debug, warn};

use super::{
    CompositionError, CompositionSuccess, FederationUpdaterConfig,
    runner::{CompositionRunner, Runner},
    supergraph::{
        config::{
            error::ResolveSubgraphError,
            full::introspect::ResolveIntrospectSubgraphFactory,
            resolver::{
                DefaultSubgraphDefinition, LoadRemoteSubgraphsError, LoadSupergraphConfigError,
                ResolveSupergraphConfigError, SupergraphConfigResolver,
                fetch_remote_subgraph::FetchRemoteSubgraphFactory,
                fetch_remote_subgraphs::FetchRemoteSubgraphsRequest,
            },
        },
        install::{InstallSupergraph, InstallSupergraphError},
    },
};
use crate::{
    command::install::{Plugin, federation_version},
    composition::supergraph::config::{
        full::FullyResolvedSupergraphConfig, lazy::LazilyResolvedSupergraphConfig,
    },
    config::SupergraphConfigYaml,
    federation::{FederationOneUnsupported, reject_federation_one},
    options::LicenseAccepter,
    plugin::{
        automatic::AutomaticDownloads,
        discovery::ManifestDirs,
        error::{PluginFailure, RequestOrigin},
        layering::LayeredDeclarations,
        precedence::{
            self, ManifestOverridden, PluginRequest, RequestInputs, declarations_in_scope,
        },
        version::PluginName,
    },
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::ExecCommand, install::InstallBinary, read_stdin::ReadStdin, write_file::WriteFile,
        },
        parsers::FileDescriptorType,
    },
};

#[derive(thiserror::Error, Debug)]
pub enum CompositionPipelineError {
    #[error("Failed to load remote subgraphs")]
    LoadRemoteSubgraphs(#[from] LoadRemoteSubgraphsError),
    #[error("Failed to load the supergraph config")]
    LoadSupergraphConfig(#[from] LoadSupergraphConfigError),
    #[error("Failed to resolve the supergraph config")]
    ResolveSupergraphConfig(#[from] ResolveSupergraphConfigError),
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("Serialization error")]
    SerdeYaml(#[from] serde_yaml::Error),
    #[error("Error writing file: {}.\n{}", .path, .err)]
    WriteFile {
        path: Utf8PathBuf,
        err: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to install the supergraph binary")]
    InstallSupergraph(#[from] InstallSupergraphError),
    #[error("Failed to resolve subgraphs:\n{}", ::itertools::join(.0.iter().map(|(name, err)| format!("{name}: {}", ::rover_std::format_error_chain(err))), "\n"))]
    ResolveSubgraphs(HashMap<String, ResolveSubgraphError>),
    #[error("Failed to resolve subgraph from prompt:\n{}", ::rover_std::format_error_chain(.0))]
    ResolveSubgraphFromPrompt(ResolveSubgraphError),
    #[error(transparent)]
    FederationOneUnsupported(#[from] FederationOneUnsupported),
    #[error("Couldn't decide which `supergraph` plugin version to use")]
    Plugin(#[source] Box<PluginFailure>),
}

pub struct CompositionPipeline<State> {
    pub(crate) state: State,
}

impl Default for CompositionPipeline<state::Init> {
    fn default() -> Self {
        CompositionPipeline { state: state::Init }
    }
}

impl CompositionPipeline<state::Init> {
    pub async fn init<S>(
        self,
        read_stdin_impl: &mut impl ReadStdin,
        fetch_remote_subgraphs_factory: S,
        supergraph_yaml: Option<FileDescriptorType>,
        graph_ref: Option<GraphRef>,
        default_subgraph: Option<DefaultSubgraphDefinition>,
    ) -> Result<CompositionPipeline<state::ResolveFederationVersion>, CompositionPipelineError>
    where
        S: MakeService<
                (),
                FetchRemoteSubgraphsRequest,
                Response = BTreeMap<String, SubgraphConfig>,
            >,
        S::MakeError: std::error::Error + Send + Sync + 'static,
        S::Error: std::error::Error + Send + Sync + 'static,
    {
        let supergraph_yaml = supergraph_yaml.and_then(|supergraph_yaml| match supergraph_yaml {
            FileDescriptorType::File(file) => canonicalize(file)
                .ok()
                .map(|file| FileDescriptorType::File(Utf8PathBuf::from_path_buf(file).unwrap())),
            FileDescriptorType::Stdin => Some(FileDescriptorType::Stdin),
        });
        let supergraph_root = supergraph_yaml
            .as_ref()
            .and_then(|file| match file {
                FileDescriptorType::File(file) => {
                    let mut current_dir =
                        current_dir().expect("Unable to get current directory path");

                    current_dir.push(file);
                    let path = Utf8PathBuf::from_path_buf(current_dir).unwrap();
                    let parent = path.parent().unwrap().to_path_buf();
                    Some(parent)
                }
                FileDescriptorType::Stdin => None,
            })
            .unwrap_or_else(|| {
                Utf8PathBuf::from_path_buf(
                    current_dir().expect("Unable to get current directory path"),
                )
                .unwrap()
            });
        eprintln!("merging supergraph schema files");
        let resolver = SupergraphConfigResolver::load_remote_subgraphs(
            fetch_remote_subgraphs_factory,
            graph_ref.as_ref(),
        )
        .await?
        .load_from_file_descriptor(read_stdin_impl, supergraph_yaml.as_ref())?;
        let resolver = match default_subgraph {
            Some(default_subgraph) => resolver
                .define_default_subgraph_if_empty(default_subgraph)
                .map_err(CompositionPipelineError::ResolveSubgraphFromPrompt)?,
            None => resolver.skip_default_subgraph(),
        };
        Ok(CompositionPipeline {
            state: state::ResolveFederationVersion {
                resolver,
                supergraph_root,
                supergraph_yaml,
            },
        })
    }
}

impl CompositionPipeline<state::ResolveFederationVersion> {
    pub async fn resolve_federation_version(
        self,
        resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        passed_in_fed_version: Option<(FederationVersion, RequestOrigin)>,
        plugin_levels: &ManifestDirs,
        warn_on_floating_version: bool,
    ) -> Result<CompositionPipeline<state::InstallSupergraph>, CompositionPipelineError> {
        // Before anything reaches the network: a manifest that can't be used,
        // or a lockfile that no longer records it, fails the command whatever
        // its version ends up being taken from.
        let declarations =
            declarations_in_scope(plugin_levels).map_err(CompositionPipelineError::Plugin)?;

        // Reject an explicit Federation 1 pin (from the CLI flag, or from `supergraph.yaml`)
        // up front, before subgraph resolution runs. A Fed-1-pin-vs-Fed-2-subgraph mismatch
        // coming out of `fully_resolve_subgraphs` below is caught and defaulted to Fed 2 (see
        // the `Err` arm), which would otherwise let a `supergraph.yaml` pin slip past the
        // `reject_federation_one` check further down uncontested.
        if let Some(user_specified_fed_version) = passed_in_fed_version
            .clone()
            .map(|(version, _)| version)
            .or_else(|| self.state.resolver.target_federation_version())
        {
            reject_federation_one(&user_specified_fed_version)?;
        }

        let resolved = self
            .state
            .resolver
            .fully_resolve_subgraphs(
                resolve_introspect_subgraph_factory.clone(),
                fetch_remote_subgraph_factory.clone(),
                &self.state.supergraph_root,
            )
            .await;
        if let Err(err) = &resolved {
            warn!("Could not fully resolve SupergraphConfig to discover Federation Version: {err}");
        }
        let (from_config, config_path) =
            config_pin(self.state.resolver.target_federation_version(), &resolved);

        let request = supergraph_request(
            passed_in_fed_version,
            from_config,
            config_path,
            plugin_levels,
            &declarations,
        )
        .map_err(CompositionPipelineError::Plugin)?;
        if resolved.is_err() {
            warnln!("{}", undetected(&request));
        }
        ManifestOverridden::process().warn_once(
            &rover_print::print::stderr::default(),
            &request,
            &declarations,
        );
        let federation_version =
            federation_version(&request.request).map_err(CompositionPipelineError::Plugin)?;
        let federation_version_origin = request.origin();

        // A flag or `supergraph.yaml` asking for Federation 1 was refused above;
        // a manifest asking for it is refused here.
        reject_federation_one(&federation_version)?;

        // Nudge users to pin an exact federation version. Composing against a
        // floating version can pull in breaking changes when a new federation release ships.
        if warn_on_floating_version && federation_version.get_exact().is_none() {
            warnln!(
                "{} isn't pinned to an exact version, so each run composes with the latest release \
                 available at the time. A new federation release can change your supergraph schema, \
                 or need a newer router than you're running. Pin an exact version (e.g. {}) and \
                 update your router before raising it. See {} for more information.",
                Style::Command.paint("federation_version"),
                Style::Command.paint("federation_version: =2.x.y"),
                Style::Link.paint(
                    "https://www.apollographql.com/docs/rover/commands/supergraphs#setting-a-composition-version"
                ),
            );
        }

        debug!("Using Federation Version '{federation_version}'");

        Ok(CompositionPipeline {
            state: state::InstallSupergraph {
                resolver: self.state.resolver,
                supergraph_root: self.state.supergraph_root,
                fetch_remote_subgraph_factory,
                federation_version,
                federation_version_origin,
                resolve_introspect_subgraph_factory,
                manifest_declarations: declarations,
            },
        })
    }
}

/// What `supergraph.yaml` asks for, `pinned`, and the file it was read from,
/// given how resolving its subgraphs went. The pin stands whether or not they
/// resolved: failing to check it against the subgraphs is no reason to let a
/// manifest or the default take its place.
fn config_pin<T, E>(
    pinned: Option<FederationVersion>,
    resolved: &Result<(FullyResolvedSupergraphConfig, T), E>,
) -> (Option<FederationVersion>, Option<Utf8PathBuf>) {
    let path = resolved
        .as_ref()
        .ok()
        .and_then(|(config, _)| config.origin_path.clone());
    (pinned, path)
}

/// What to say when the subgraphs couldn't be resolved to detect the
/// Federation version from, naming where the version used instead came from.
fn undetected(request: &PluginRequest) -> String {
    match request.origin() {
        None => format!("Federation Version could not be detected, defaulting to: {LatestFedTwo}"),
        Some(origin) => format!(
            "Federation Version could not be detected from the subgraphs, so using {}, {origin}.",
            request.request
        ),
    }
}

/// The `supergraph` request to compose with: `passed_in` from a flag or its
/// environment variable, then `from_config` from the supergraph config at
/// `config_path`, then the manifests, then the newest Federation 2 release.
fn supergraph_request(
    passed_in: Option<(FederationVersion, RequestOrigin)>,
    from_config: Option<FederationVersion>,
    config_path: Option<Utf8PathBuf>,
    plugin_levels: &ManifestDirs,
    declarations: &LayeredDeclarations,
) -> Result<PluginRequest, Box<PluginFailure>> {
    let as_request = |version| Plugin::Supergraph(version).request();
    let inputs = RequestInputs::new(as_request(LatestFedTwo))
        .with_override(passed_in.map(|(version, origin)| (as_request(version), origin)))
        .with_supergraph_config(from_config.map(as_request), config_path);

    precedence::request(PluginName::Supergraph, inputs, plugin_levels, declarations)
}

impl CompositionPipeline<state::InstallSupergraph> {
    pub async fn install_supergraph_binary(
        self,
        studio_client_config: StudioClientConfig,
        override_install_path: Option<Utf8PathBuf>,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
    ) -> Result<CompositionPipeline<state::Run>, CompositionPipelineError> {
        let supergraph_binary =
            InstallSupergraph::new(self.state.federation_version, studio_client_config)
                .requested_by(self.state.federation_version_origin)
                .automatic_downloads(AutomaticDownloads::in_scope(
                    &self.state.manifest_declarations,
                ))
                .install(override_install_path, elv2_license_accepter, skip_update)
                .await;

        Ok(CompositionPipeline {
            state: state::Run {
                resolver: self.state.resolver,
                supergraph_root: self.state.supergraph_root,
                supergraph_binary,
                resolve_introspect_subgraph_factory: self.state.resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory: self.state.fetch_remote_subgraph_factory,
                manifest_declarations: self.state.manifest_declarations,
            },
        })
    }
}

impl CompositionPipeline<state::Run> {
    pub async fn compose(
        &self,
        exec_command_impl: &impl ExecCommand,
        write_file_impl: &impl WriteFile,
    ) -> Result<CompositionSuccess, CompositionError> {
        // Everything below this line runs with the plugin already resolved and
        // its FR54 line already on stderr, so every failure here names it
        // (FR57). `None` only when resolution itself failed — which is the
        // error `supergraph_binary.clone()?` surfaces at the end.
        let provenance = || {
            self.state
                .supergraph_binary
                .as_ref()
                .ok()
                .map(|binary| binary.provenance().boxed())
        };

        let supergraph_config_filepath = Utf8PathBuf::from_path_buf(
            tempdir()
                .map_err(|source| CompositionError::TempDir {
                    source,
                    provenance: provenance(),
                })?
                .path()
                .join("supergraph.yaml"),
        )
        .expect("Unable to parse path");

        let (fully_resolved_supergraph_config, errors) = self
            .state
            .resolver
            .fully_resolve_subgraphs(
                self.state.resolve_introspect_subgraph_factory.clone(),
                self.state.fetch_remote_subgraph_factory.clone(),
                &self.state.supergraph_root,
            )
            .await
            .map_err(|source| CompositionError::ResolvingSubgraphsError {
                source,
                provenance: provenance(),
            })?;

        if !errors.is_empty() {
            return Err(CompositionError::ResolvingSubgraphsError {
                source: ResolveSupergraphConfigError::ResolveSubgraphs(errors),
                provenance: provenance(),
            });
        }

        write_file_impl
            .write_file(
                &supergraph_config_filepath,
                serde_yaml::to_string(&SupergraphConfigYaml::from(
                    fully_resolved_supergraph_config,
                ))
                .map_err(|source| CompositionError::SerdeYaml {
                    source,
                    provenance: provenance(),
                })?
                .as_bytes(),
            )
            .await
            .map_err(|err| CompositionError::WriteFile {
                path: supergraph_config_filepath.clone(),
                error: Box::new(err),
                provenance: provenance(),
            })?;

        self.state
            .supergraph_binary
            .clone()?
            .compose(exec_command_impl, supergraph_config_filepath)
            .await
    }

    #[tracing::instrument(skip_all)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn runner<ExecC, WriteF>(
        &self,
        exec_command: ExecC,
        write_file: WriteF,
        http_service: HttpService,
        make_fetch_remote_subgraph: FetchRemoteSubgraphFactory,
        introspection_polling_interval: u64,
        output_dir: Utf8PathBuf,
        compose_on_initialisation: bool,
        federation_updater_config: Option<FederationUpdaterConfig>,
    ) -> Result<CompositionRunner<ExecC, WriteF>, CompositionPipelineError>
    where
        ExecC: ExecCommand + Debug + Eq + PartialEq + Send + Sync + 'static,
        WriteF: WriteFile + Debug + Eq + PartialEq + Send + Sync + 'static,
    {
        // We want to filter down the subgraphs we have at this point,
        // so we want to lazily resolve, and track any subgraphs that won't do that
        // followed by fully resolving and then tracking any subgraphs that won't do that either.
        //
        // The set of subgraphs that will fully resolve, will form our initial set, and
        // then we can return a stream that's been set up as best as possible, with as many subgraphs
        // as we can.
        let (
            lazily_resolved_supergraph_config,
            fully_resolved_supergraph_config,
            resolution_errors,
        ) = self
            .generate_lazy_and_fully_resolved_supergraph_configs()
            .await?;

        let subgraphs = lazily_resolved_supergraph_config.subgraphs().clone();

        let runner = Runner::default()
            .setup_subgraph_watchers(
                subgraphs,
                http_service,
                make_fetch_remote_subgraph,
                self.state.supergraph_root.clone(),
                introspection_polling_interval,
            )
            .await
            .map_err(CompositionPipelineError::ResolveSubgraphs)?
            .setup_supergraph_config_watcher(
                lazily_resolved_supergraph_config,
                self.state.resolver.remote_subgraphs().clone(),
                self.state.fetch_remote_subgraph_factory.clone(),
                self.state.resolve_introspect_subgraph_factory.clone(),
            )
            .setup_composition_watcher(
                fully_resolved_supergraph_config,
                resolution_errors,
                self.state.supergraph_binary.clone(),
                exec_command,
                write_file,
                output_dir,
                compose_on_initialisation,
                federation_updater_config,
                self.state.manifest_declarations.clone(),
            );
        Ok(runner)
    }

    #[tracing::instrument(skip_all)]
    async fn generate_lazy_and_fully_resolved_supergraph_configs(
        &self,
    ) -> Result<
        (
            LazilyResolvedSupergraphConfig,
            FullyResolvedSupergraphConfig,
            BTreeMap<String, ResolveSubgraphError>,
        ),
        CompositionPipelineError,
    > {
        tracing::debug!("generate_lazy_and_fully_resolved_supergraph_configs");
        // Get the two different kinds of resolutions (we know that the fully_resolved will be a non-proper subset of the lazily_resolved)
        let (lazily_resolved_supergraph_config, _) = self
            .state
            .resolver
            .lazily_resolve_subgraphs(&self.state.supergraph_root)
            .await?;
        debug!(
            "Lazily Resolved Config is: {:?}",
            lazily_resolved_supergraph_config
        );
        let (fully_resolved_supergraph_config, full_resolution_errors) = self
            .state
            .resolver
            .fully_resolve_subgraphs(
                self.state.resolve_introspect_subgraph_factory.clone(),
                self.state.fetch_remote_subgraph_factory.clone(),
                &self.state.supergraph_root,
            )
            .await?;
        debug!(
            "Fully Resolved Config is: {:?}",
            fully_resolved_supergraph_config
        );

        // Note: subgraphs that failed full resolution (e.g. unreachable introspection endpoints)
        // are intentionally kept in the lazily-resolved config so that watchers are created for
        // them. Those watchers will keep polling and trigger recomposition once the subgraphs
        // become available.
        Ok((
            lazily_resolved_supergraph_config,
            fully_resolved_supergraph_config,
            full_resolution_errors,
        ))
    }
}

pub(crate) mod state {
    use apollo_federation_types::config::FederationVersion;
    use camino::Utf8PathBuf;

    use crate::{
        composition::supergraph::{
            binary::SupergraphBinary,
            config::{
                full::introspect::ResolveIntrospectSubgraphFactory,
                resolver::{
                    InitializedSupergraphConfigResolver,
                    fetch_remote_subgraph::FetchRemoteSubgraphFactory,
                },
            },
            install::InstallSupergraphError,
        },
        plugin::{error::RequestOrigin, layering::LayeredDeclarations},
        utils::parsers::FileDescriptorType,
    };

    pub struct Init;
    pub struct ResolveFederationVersion {
        pub resolver: InitializedSupergraphConfigResolver,
        pub supergraph_root: Utf8PathBuf,
        pub supergraph_yaml: Option<FileDescriptorType>,
    }
    pub struct InstallSupergraph {
        pub resolver: InitializedSupergraphConfigResolver,
        pub supergraph_root: Utf8PathBuf,
        pub federation_version: FederationVersion,
        pub federation_version_origin: Option<RequestOrigin>,
        pub resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        pub fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        /// The manifests' declarations, which a mid-session change to
        /// `supergraph.yaml` is weighed against.
        pub manifest_declarations: LayeredDeclarations,
    }
    pub struct Run {
        pub resolver: InitializedSupergraphConfigResolver,
        pub supergraph_root: Utf8PathBuf,
        pub supergraph_binary: Result<SupergraphBinary, InstallSupergraphError>,
        pub resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        pub fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        pub manifest_declarations: LayeredDeclarations,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rover_std::format_error_chain;
    use speculoos::prelude::*;

    use super::*;
    use crate::composition::supergraph::config::scenario::fed_one_pin_with_fed_two_subgraph_resolver;

    fn exact(minor: u64) -> FederationVersion {
        FederationVersion::ExactFedTwo(semver::Version::new(2, minor, 3))
    }

    const FLAG: RequestOrigin = RequestOrigin::Flag("--federation-version");
    const ENV: RequestOrigin = RequestOrigin::EnvVar("APOLLO_ROVER_DEV_COMPOSITION_VERSION");
    const PINNED: &str = "plugins:\n  supergraph: \"=2.8.3\"\n";
    const FLOATING: &str = "plugins:\n  supergraph: \"2\"\n";

    /// Each of the places composition takes its version from, against a
    /// project whose manifest declares `manifest`, with a lockfile pinning
    /// `supergraph` at 2.5.3 beside it.
    #[rstest::rstest]
    #[case::a_flag(Some((exact(9), FLAG)), Some(exact(7)), PINNED, (exact(9), Some(FLAG)))]
    #[case::its_variable(Some((exact(9), ENV)), Some(exact(7)), PINNED, (exact(9), Some(ENV)))]
    #[case::the_config(
        None,
        Some(exact(7)),
        PINNED,
        (exact(7), Some(RequestOrigin::SupergraphConfig(Some("graphs/prod.yaml".into()))))
    )]
    #[case::the_manifest(None, None, PINNED, (exact(8), Some(RequestOrigin::Manifest)))]
    #[case::the_manifest_pinned_by_its_lockfile(
        None,
        None,
        FLOATING,
        (exact(5), Some(RequestOrigin::Lockfile))
    )]
    #[case::the_default(None, None, "", (LatestFedTwo, None))]
    fn the_version_names_where_it_came_from(
        #[case] passed_in: Option<(FederationVersion, RequestOrigin)>,
        #[case] from_config: Option<FederationVersion>,
        #[case] manifest: &str,
        #[case] expected: (FederationVersion, Option<RequestOrigin>),
    ) {
        let temp = tempfile::tempdir().unwrap();
        let project = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        std::fs::write(project.join("rover.yaml"), manifest).unwrap();
        std::fs::write(
            project.join("plugin-versions.lock"),
            "version = 1\n\n[[plugins]]\nname = \"supergraph\"\nrequested = \"2\"\nresolved = \"2.5.3\"\n",
        )
        .unwrap();
        let levels = ManifestDirs {
            global: None,
            project: Some(project),
        };
        let declarations = LayeredDeclarations::load(&levels).unwrap();

        let request = supergraph_request(
            passed_in,
            from_config,
            Some("graphs/prod.yaml".into()),
            &levels,
            &declarations,
        )
        .unwrap();

        assert_that!((
            federation_version(&request.request).unwrap(),
            request.origin()
        ))
        .is_equal_to(expected);
    }

    /// A pin in `supergraph.yaml` stands even when the subgraphs can't be
    /// resolved, rather than falling through to a manifest or the default.
    #[rstest::rstest]
    #[case::resolved(Ok::<_, ()>((
        FullyResolvedSupergraphConfig::builder()
            .subgraphs(BTreeMap::new())
            .federation_version(exact(7))
            .origin_path(Utf8PathBuf::from("graphs/prod.yaml"))
            .build(),
        (),
    )), Some(Utf8PathBuf::from("graphs/prod.yaml")))]
    #[case::unresolved(Err(()), None)]
    fn a_pin_stands_however_its_subgraphs_resolve(
        #[case] resolved: Result<(FullyResolvedSupergraphConfig, ()), ()>,
        #[case] path: Option<Utf8PathBuf>,
    ) {
        assert_that!(config_pin(Some(exact(7)), &resolved)).is_equal_to((Some(exact(7)), path));
    }

    #[rstest::rstest]
    #[case::the_default(None, "Federation Version could not be detected, defaulting to: 2")]
    #[case::a_manifest(
        Some(RequestOrigin::Manifest),
        "Federation Version could not be detected from the subgraphs, so using =2.8.3, declared \
         in `rover.yaml`."
    )]
    #[case::supergraph_yaml(
        Some(RequestOrigin::SupergraphConfig(None)),
        "Federation Version could not be detected from the subgraphs, so using =2.8.3, set by \
         `federation_version` in the supergraph config."
    )]
    fn an_undetected_version_says_where_the_one_used_came_from(
        #[case] origin: Option<RequestOrigin>,
        #[case] expected: &str,
    ) {
        use crate::plugin::{
            layering::DeclarationLevel, precedence::RequestSource, version::VersionRequest,
        };

        let (request, source) = match origin {
            None => (VersionRequest::Major(2), RequestSource::Default),
            Some(RequestOrigin::Manifest) => (
                VersionRequest::Exact(semver::Version::new(2, 8, 3)),
                RequestSource::Manifest(DeclarationLevel::Project),
            ),
            Some(_) => (
                VersionRequest::Exact(semver::Version::new(2, 8, 3)),
                RequestSource::SupergraphConfig(None),
            ),
        };
        let request = PluginRequest {
            plugin: PluginName::Supergraph,
            request,
            source,
        };

        assert_that!(undetected(&request)).is_equal_to(expected.to_string());
    }

    #[test]
    fn load_remote_subgraphs_message_excludes_its_cause() {
        let err =
            CompositionPipelineError::from(LoadRemoteSubgraphsError::FetchRemoteSubgraphsError(
                Box::<dyn std::error::Error + Send + Sync>::from(
                    "the registry refused the request",
                ),
            ));

        assert_that!(err.to_string()).is_equal_to("Failed to load remote subgraphs".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(
            "Failed to load remote subgraphs: the registry refused the request".to_string(),
        );
    }

    #[test]
    fn load_supergraph_config_message_excludes_its_cause() {
        let deserialize_err = serde_yaml::from_str::<serde_yaml::Value>("[1, 2").unwrap_err();
        let cause = deserialize_err.to_string();
        let err = CompositionPipelineError::from(LoadSupergraphConfigError::DeserializationError(
            deserialize_err,
        ));

        assert_that!(err.to_string())
            .is_equal_to("Failed to load the supergraph config".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(format!(
            "Failed to load the supergraph config: Failed to deserialise the supergraph config: {cause}"
        ));
    }

    #[test]
    fn resolve_supergraph_config_message_excludes_its_cause() {
        let err = CompositionPipelineError::from(ResolveSupergraphConfigError::NoSource);

        assert_that!(err.to_string())
            .is_equal_to("Failed to resolve the supergraph config".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(
            "Failed to resolve the supergraph config: No source found for supergraph config"
                .to_string(),
        );
    }

    #[test]
    fn io_message_excludes_its_cause() {
        let err = CompositionPipelineError::from(std::io::Error::other("the disk went away"));

        assert_that!(err.to_string()).is_equal_to("IO error".to_string());
        assert_that!(format_error_chain(&err))
            .is_equal_to("IO error: the disk went away".to_string());
    }

    #[test]
    fn serde_yaml_message_excludes_its_cause() {
        let inner = serde_yaml::from_str::<serde_yaml::Value>("[1, 2").unwrap_err();
        let cause = inner.to_string();
        let err = CompositionPipelineError::from(inner);

        assert_that!(err.to_string()).is_equal_to("Serialization error".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(format!("Serialization error: {cause}"));
    }

    #[test]
    fn install_supergraph_message_excludes_its_cause() {
        let err = CompositionPipelineError::from(InstallSupergraphError::MissingDependency {
            err: "the plugin was not installed".to_string(),
        });

        assert_that!(err.to_string())
            .is_equal_to("Failed to install the supergraph binary".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(
            "Failed to install the supergraph binary: unable to find dependency: \"the plugin was not installed\""
                .to_string(),
        );
    }

    /// The map is not a `source`, so nothing downstream walks into these errors — this variant's
    /// own message is the only place a subgraph's reason can appear.
    #[test]
    fn resolve_subgraphs_renders_each_entrys_cause() {
        let err = CompositionPipelineError::ResolveSubgraphs(HashMap::from([(
            "products".to_string(),
            ResolveSubgraphError::IntrospectionError {
                subgraph_name: "products".to_string(),
                source: Arc::new(Box::from("connection refused")),
            },
        )]));

        assert_that!(err.to_string()).is_equal_to(
            "Failed to resolve subgraphs:\nproducts: Failed to introspect the subgraph \"products\": connection refused"
                .to_string(),
        );
    }

    #[test]
    fn resolve_subgraph_from_prompt_renders_its_cause() {
        let err = CompositionPipelineError::ResolveSubgraphFromPrompt(
            ResolveSubgraphError::FetchRemoteSdlError {
                subgraph_name: "products".to_string(),
                source: Arc::new(Box::from("the registry refused the request")),
            },
        );

        assert_that!(err.to_string()).is_equal_to(
            "Failed to resolve subgraph from prompt:\nFailed to fetch the sdl for subgraph `products` from remote: the registry refused the request"
                .to_string(),
        );
    }

    /// This pins the regression the up-front `target_federation_version()` check in
    /// `resolve_federation_version` guards against: without it, a `federation_version: 1` pin
    /// in `supergraph.yaml` alongside a Federation-2 (`@link`-using) subgraph gets caught by the
    /// `Err` arm below (`fully_resolve_subgraphs` returns `FederationVersionMismatch`) and
    /// silently defaulted to `LatestFedTwo`, instead of being rejected.
    #[tokio::test]
    async fn resolve_federation_version_rejects_fed_one_pin_with_fed_two_subgraph() {
        let (
            resolver,
            resolve_introspect_subgraph_factory,
            fetch_remote_subgraph_factory,
            supergraph_root,
            _tmp,
        ) = fed_one_pin_with_fed_two_subgraph_resolver().await;

        let pipeline = CompositionPipeline {
            state: state::ResolveFederationVersion {
                resolver,
                supergraph_root,
                supergraph_yaml: None,
            },
        };

        // No CLI flag override -- forces `resolve_federation_version` to read the
        // `supergraph.yaml` pin via `target_federation_version()`.
        let result = pipeline
            .resolve_federation_version(
                resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory,
                None,
                &ManifestDirs {
                    global: None,
                    project: None,
                },
                false,
            )
            .await;

        // `CompositionPipeline<state::InstallSupergraph>` (the `Ok` type here) isn't `Debug` --
        // it holds non-`Debug` Tower service factories -- so `speculoos`'s `ResultAssertions`
        // (which requires both sides of the `Result` to be `Debug`) can't be used here; a plain
        // `matches!` needs no such bound.
        assert!(matches!(
            result,
            Err(CompositionPipelineError::FederationOneUnsupported(_))
        ));
    }
}
