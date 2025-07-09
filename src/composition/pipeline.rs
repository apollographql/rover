use std::collections::{BTreeMap, HashMap};
use std::env::current_dir;
use std::fmt::Debug;
use std::fs::canonicalize;

use apollo_federation_types::config::FederationVersion::LatestFedTwo;
use apollo_federation_types::config::{FederationVersion, SubgraphConfig};
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use rover_http::HttpService;
use rover_std::warnln;
use tempfile::tempdir;
use tower::MakeService;
use tracing::{debug, warn};

use super::runner::{CompositionRunner, Runner};
use super::supergraph::config::error::ResolveSubgraphError;
use super::supergraph::config::full::introspect::ResolveIntrospectSubgraphFactory;
use super::supergraph::config::resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory;
use super::supergraph::config::resolver::fetch_remote_subgraphs::FetchRemoteSubgraphsRequest;
use super::supergraph::config::resolver::{
    DefaultSubgraphDefinition, LoadRemoteSubgraphsError, LoadSupergraphConfigError,
    ResolveSupergraphConfigError, SupergraphConfigResolver,
};
use super::supergraph::install::{InstallSupergraph, InstallSupergraphError};
use super::{CompositionError, CompositionSuccess, FederationUpdaterConfig};
use crate::composition::supergraph::config::SupergraphConfigYaml;
use crate::composition::supergraph::config::full::FullyResolvedSupergraphConfig;
use crate::composition::supergraph::config::lazy::LazilyResolvedSupergraphConfig;
use crate::options::LicenseAccepter;
use crate::utils::client::StudioClientConfig;
use crate::utils::effect::exec::ExecCommand;
use crate::utils::effect::install::InstallBinary;
use crate::utils::effect::read_stdin::ReadStdin;
use crate::utils::effect::write_file::WriteFile;
use crate::utils::parsers::FileDescriptorType;

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
    ) -> CompositionPipeline<state::InstallSupergraph> {
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

        debug!("Using Federation Version '{federation_version}'");

        CompositionPipeline {
            state: state::InstallSupergraph {
                resolver: self.state.resolver,
                supergraph_root: self.state.supergraph_root,
                fetch_remote_subgraph_factory,
                federation_version,
                resolve_introspect_subgraph_factory,
            },
        }
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
        let (mut lazily_resolved_supergraph_config, _) = self
            .state
            .resolver
            .lazily_resolve_subgraphs(&self.state.supergraph_root)
            .await?;
        debug!(
            "Initial Lazily Resolved Config is: {:?}",
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
            "Initial Fully Resolved Config is: {:?}",
            fully_resolved_supergraph_config
        );
        // Generate the correct lazily_resolved config, by removing all the things that cannot fully resolve
        lazily_resolved_supergraph_config
            .filter_subgraphs(full_resolution_errors.keys().cloned().collect());
        debug!("Final Config is: {:?}", lazily_resolved_supergraph_config);

        // Merge all the errors together and give all three back
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

    use crate::composition::supergraph::binary::SupergraphBinary;
    use crate::composition::supergraph::config::full::introspect::ResolveIntrospectSubgraphFactory;
    use crate::composition::supergraph::config::resolver::InitializedSupergraphConfigResolver;
    use crate::composition::supergraph::config::resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory;
    use crate::composition::supergraph::install::InstallSupergraphError;
    use crate::utils::parsers::FileDescriptorType;

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
