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
    composition::supergraph::config::{
        full::FullyResolvedSupergraphConfig, lazy::LazilyResolvedSupergraphConfig,
    },
    config::SupergraphConfigYaml,
    federation::{FederationOneUnsupported, reject_federation_one},
    options::LicenseAccepter,
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
    #[error("Failed to load remote subgraphs.\n{}", .0)]
    LoadRemoteSubgraphs(#[from] LoadRemoteSubgraphsError),
    #[error("Failed to load the supergraph config.\n{}", .0)]
    LoadSupergraphConfig(#[from] LoadSupergraphConfigError),
    #[error("Failed to resolve the supergraph config.\n{}", .0)]
    ResolveSupergraphConfig(#[from] ResolveSupergraphConfigError),
    #[error("IO error.\n{}", .0)]
    Io(#[from] std::io::Error),
    #[error("Serialization error.\n{}", .0)]
    SerdeYaml(#[from] serde_yaml::Error),
    #[error("Error writing file: {}.\n{}", .path, .err)]
    WriteFile {
        path: Utf8PathBuf,
        err: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to install the supergraph binary.\n{}", .0)]
    InstallSupergraph(#[from] InstallSupergraphError),
    #[error("Failed to resolve subgraphs:\n{}", ::itertools::join(.0.iter().map(|(name, err)| format!("{name}: {err}")), "\n"))]
    ResolveSubgraphs(HashMap<String, ResolveSubgraphError>),
    #[error("Failed to resolve subgraph from prompt:\n{}", .0)]
    ResolveSubgraphFromPrompt(ResolveSubgraphError),
    #[error(transparent)]
    FederationOneUnsupported(#[from] FederationOneUnsupported),
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
        passed_in_fed_version: Option<FederationVersion>,
        warn_on_floating_version: bool,
    ) -> Result<CompositionPipeline<state::InstallSupergraph>, CompositionPipelineError> {
        // Reject an explicit Federation 1 pin (from the CLI flag, or from `supergraph.yaml`)
        // up front, before subgraph resolution runs. A Fed-1-pin-vs-Fed-2-subgraph mismatch
        // coming out of `fully_resolve_subgraphs` below is caught and defaulted to Fed 2 (see
        // the `Err` arm), which would otherwise let a `supergraph.yaml` pin slip past the
        // `reject_federation_one` check further down uncontested.
        let user_specified_fed_version = passed_in_fed_version
            .clone()
            .or_else(|| self.state.resolver.target_federation_version());
        if let Some(user_specified_fed_version) = &user_specified_fed_version {
            reject_federation_one(user_specified_fed_version)?;
        }

        let resolved_federation_version = match self
            .state
            .resolver
            .fully_resolve_subgraphs(
                resolve_introspect_subgraph_factory.clone(),
                fetch_remote_subgraph_factory.clone(),
                &self.state.supergraph_root,
            )
            .await
        {
            Ok((fully_resolved_supergraph_config, _)) => {
                fully_resolved_supergraph_config.federation_version
            }
            Err(err) => {
                warn!(
                    "Could not fully resolve SupergraphConfig to discover Federation Version: {err}"
                );
                warn!("Defaulting to Federation Version: {LatestFedTwo}");
                warnln!("Federation Version could not be detected, defaulting to: {LatestFedTwo}");
                LatestFedTwo
            }
        };

        let federation_version = if let Some(fed_version) = passed_in_fed_version {
            fed_version
        } else {
            resolved_federation_version
        };

        reject_federation_one(&federation_version)?;

        // Nudge users to pin an exact federation version. Composing against a
        // floating version can pull in breaking changes when a new federation release ships.
        if warn_on_floating_version && federation_version.get_exact().is_none() {
            warnln!(
                "An exact {} was not specified in your supergraph configuration. Future versions of {} \
                 will fail without an exact federation version. Pin one (e.g. {}) to prevent breaking \
                 changes, and make sure to update your router before increasing your composition version. \
                 See {} for more information.",
                Style::Command.paint("federation_version"),
                Style::Command.paint("rover supergraph compose"),
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
                resolve_introspect_subgraph_factory,
            },
        })
    }
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
                .install(override_install_path, elv2_license_accepter, skip_update)
                .await;

        Ok(CompositionPipeline {
            state: state::Run {
                resolver: self.state.resolver,
                supergraph_root: self.state.supergraph_root,
                supergraph_binary,
                resolve_introspect_subgraph_factory: self.state.resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory: self.state.fetch_remote_subgraph_factory,
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
        let supergraph_config_filepath =
            Utf8PathBuf::from_path_buf(tempdir()?.path().join("supergraph.yaml"))
                .expect("Unable to parse path");

        let (fully_resolved_supergraph_config, errors) = self
            .state
            .resolver
            .fully_resolve_subgraphs(
                self.state.resolve_introspect_subgraph_factory.clone(),
                self.state.fetch_remote_subgraph_factory.clone(),
                &self.state.supergraph_root,
            )
            .await?;

        if !errors.is_empty() {
            return Err(CompositionError::ResolvingSubgraphsError(
                ResolveSupergraphConfigError::ResolveSubgraphs(errors),
            ));
        }

        write_file_impl
            .write_file(
                &supergraph_config_filepath,
                serde_yaml::to_string(&SupergraphConfigYaml::from(
                    fully_resolved_supergraph_config,
                ))?
                .as_bytes(),
            )
            .await
            .map_err(|err| CompositionError::WriteFile {
                path: supergraph_config_filepath.clone(),
                error: Box::new(err),
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
        pub resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        pub fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
    }
    pub struct Run {
        pub resolver: InitializedSupergraphConfigResolver,
        pub supergraph_root: Utf8PathBuf,
        pub supergraph_binary: Result<SupergraphBinary, InstallSupergraphError>,
        pub resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        pub fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use apollo_federation_types::config::SchemaSource;
    use assert_fs::{
        TempDir,
        prelude::{FileTouch, FileWriteStr, PathChild},
    };
    use mockall::predicate;
    use rover_client::RoverClientError;
    use tower::{ServiceBuilder, ServiceExt};

    use super::*;
    use crate::{
        composition::supergraph::config::{
            full::{FullyResolvedSubgraph, introspect::MakeResolveIntrospectSubgraphRequest},
            resolver::{
                fetch_remote_subgraph::{
                    FetchRemoteSubgraphError, FetchRemoteSubgraphRequest,
                    MakeFetchRemoteSubgraphError, RemoteSubgraph,
                },
                fetch_remote_subgraphs::MakeFetchRemoteSubgraphsError,
            },
            scenario::*,
        },
        utils::effect::read_stdin::MockReadStdin,
    };

    /// This pins the regression the up-front `target_federation_version()` check in
    /// `resolve_federation_version` guards against: without it, a `federation_version: 1` pin
    /// in `supergraph.yaml` alongside a Federation-2 (`@link`-using) subgraph gets caught by the
    /// `Err` arm below (`fully_resolve_subgraphs` returns `FederationVersionMismatch`) and
    /// silently defaulted to `LatestFedTwo`, instead of being rejected.
    #[tokio::test]
    async fn resolve_federation_version_rejects_fed_one_pin_with_fed_two_subgraph() {
        let subgraph_name = subgraph_name();
        let subgraph_scenario = sdl_subgraph_scenario(
            sdl_fed2(sdl()),
            subgraph_name.to_string(),
            SubgraphFederationVersion::Two,
            routing_url(),
        );

        let mut local_subgraphs = BTreeMap::new();
        setup_sdl_subgraph_scenario(Some(&subgraph_scenario), &mut local_subgraphs);

        let supergraph_config = SupergraphConfigYaml {
            subgraphs: local_subgraphs,
            federation_version: Some(FederationVersion::LatestFedOne),
        };
        let supergraph_config_str = serde_yaml::to_string(&supergraph_config).unwrap();

        let local_supergraph_config_dir = TempDir::new().expect("Couldn't create temp dir.");
        let mut mock_read_stdin = MockReadStdin::new();
        let file_descriptor_type = setup_file_descriptor(
            true,
            &local_supergraph_config_dir,
            &supergraph_config_str,
            &mut mock_read_stdin,
        )
        .expect("Couldn't setup file descriptor.");

        let (fetch_remote_subgraphs_service, _) = tower_test::mock::spawn::<
            FetchRemoteSubgraphsRequest,
            BTreeMap<String, SubgraphConfig>,
        >();
        let fetch_remote_subgraphs_factory =
            ServiceBuilder::new()
                .boxed_clone()
                .service_fn(move |_: ()| {
                    let fetch_remote_subgraphs_service = fetch_remote_subgraphs_service.clone();
                    async move {
                        Ok::<_, MakeFetchRemoteSubgraphsError>(
                            ServiceBuilder::new()
                                .map_err(RoverClientError::ServiceReady)
                                .service(fetch_remote_subgraphs_service.into_inner())
                                .boxed_clone(),
                        )
                    }
                });

        let resolver =
            SupergraphConfigResolver::load_remote_subgraphs(fetch_remote_subgraphs_factory, None)
                .await
                .expect("Couldn't load remote subgraphs.")
                .load_from_file_descriptor(&mut mock_read_stdin, Some(&file_descriptor_type))
                .expect("Couldn't load local subgraphs.")
                .skip_default_subgraph();

        let (fetch_remote_subgraph_service, _) =
            tower_test::mock::spawn::<FetchRemoteSubgraphRequest, RemoteSubgraph>();
        let fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory = ServiceBuilder::new()
            .boxed_clone()
            .service_fn(move |_: ()| {
                let fetch_remote_subgraph_service = fetch_remote_subgraph_service.clone();
                async move {
                    Ok::<_, MakeFetchRemoteSubgraphError>(
                        ServiceBuilder::new()
                            .map_err(FetchRemoteSubgraphError::Service)
                            .service(fetch_remote_subgraph_service.into_inner())
                            .boxed_clone(),
                    )
                }
            });

        let (resolve_introspect_subgraph_service, mut resolve_introspect_subgraph_handle) =
            tower_test::mock::spawn::<(), FullyResolvedSubgraph>();
        // we never introspect subgraphs in this test, but we still have to account for the effect
        resolve_introspect_subgraph_handle.allow(0);
        let resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory =
            ServiceBuilder::new().boxed_clone().service_fn(
                move |_: MakeResolveIntrospectSubgraphRequest| {
                    let resolve_introspect_subgraph_service =
                        resolve_introspect_subgraph_service.clone();
                    async move {
                        Ok(ServiceBuilder::new()
                            .boxed_clone()
                            .map_err(|err| ResolveSubgraphError::IntrospectionError {
                                subgraph_name: "dont-call-me".to_string(),
                                source: Arc::new(err),
                            })
                            .service(resolve_introspect_subgraph_service.into_inner()))
                    }
                },
            );

        let supergraph_root =
            Utf8PathBuf::from_path_buf(local_supergraph_config_dir.path().to_path_buf()).unwrap();

        let pipeline = CompositionPipeline {
            state: state::ResolveFederationVersion {
                resolver,
                supergraph_root,
                supergraph_yaml: Some(file_descriptor_type),
            },
        };

        // No CLI flag override -- forces `resolve_federation_version` to read the
        // `supergraph.yaml` pin via `target_federation_version()`.
        let result = pipeline
            .resolve_federation_version(
                resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory,
                None,
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

    fn setup_sdl_subgraph_scenario(
        sdl_subgraph_scenario: Option<&SdlSubgraphScenario>,
        local_subgraphs: &mut BTreeMap<String, SubgraphConfig>,
    ) {
        if let Some(sdl_subgraph_scenario) = sdl_subgraph_scenario {
            let schema_source = SchemaSource::Sdl {
                sdl: sdl_subgraph_scenario.sdl.to_string(),
            };
            let subgraph_config = SubgraphConfig {
                routing_url: Some(routing_url()),
                schema: schema_source,
            };
            local_subgraphs.insert("sdl-subgraph".to_string(), subgraph_config);
        }
    }

    fn setup_file_descriptor(
        load_supergraph_config_from_file: bool,
        local_supergraph_config_dir: &TempDir,
        local_supergraph_config_str: &str,
        mock_read_stdin: &mut MockReadStdin,
    ) -> Result<FileDescriptorType> {
        let file_descriptor_type = if load_supergraph_config_from_file {
            let local_supergraph_config_file = local_supergraph_config_dir.child("supergraph.yaml");
            local_supergraph_config_file.touch()?;
            local_supergraph_config_file.write_str(local_supergraph_config_str)?;
            let path =
                Utf8PathBuf::from_path_buf(local_supergraph_config_file.path().to_path_buf())
                    .unwrap();
            mock_read_stdin.expect_read_stdin().times(0);
            FileDescriptorType::File(path)
        } else {
            mock_read_stdin
                .expect_read_stdin()
                .times(1)
                .with(predicate::eq("supergraph config"))
                .returning({
                    let local_supergraph_config_str = local_supergraph_config_str.to_string();
                    move |_| Ok(local_supergraph_config_str.to_string())
                });
            FileDescriptorType::Stdin
        };
        Ok(file_descriptor_type)
    }
}
