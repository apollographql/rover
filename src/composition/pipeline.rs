use std::{
    collections::{BTreeMap, HashMap},
    env::current_dir,
    fmt::Debug,
    fs::canonicalize,
};

use apollo_federation_types::config::FederationVersion::LatestFedTwo;
use apollo_federation_types::config::{FederationVersion, SubgraphConfig, SupergraphConfig};
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use rover_http::HttpService;
use tempfile::tempdir;
use tower::MakeService;
use tracing::{info, warn};

use super::{
    runner::{CompositionRunner, Runner},
    supergraph::{
        binary::OutputTarget,
        config::{
            error::ResolveSubgraphError,
            full::introspect::ResolveIntrospectSubgraphFactory,
            resolver::{
                fetch_remote_subgraph::FetchRemoteSubgraphFactory,
                fetch_remote_subgraphs::FetchRemoteSubgraphsRequest, LoadRemoteSubgraphsError,
                LoadSupergraphConfigError, ResolveSupergraphConfigError, SubgraphPrompt,
                SupergraphConfigResolver,
            },
        },
        install::{InstallSupergraph, InstallSupergraphError},
    },
    CompositionError, CompositionSuccess, FederationUpdaterConfig,
};
use crate::composition::supergraph::config::full::FullyResolvedSupergraphConfig;
use crate::composition::supergraph::config::lazy::LazilyResolvedSupergraphConfig;
use crate::composition::supergraph::config::resolver::Prompt;
use crate::{
    options::LicenseAccepter,
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::ExecCommand, install::InstallBinary, read_file::ReadFile, read_stdin::ReadStdin,
            write_file::WriteFile,
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
    #[error("Failed to resolve subgraphs:\n{}", ::itertools::join(.0.iter().map(|(name, err)| format!("{}: {}", name, err)), "\n"))]
    ResolveSubgraphs(HashMap<String, ResolveSubgraphError>),
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
            .clone()
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
        let resolver = SupergraphConfigResolver::default()
            .load_remote_subgraphs(fetch_remote_subgraphs_factory, graph_ref.as_ref())
            .await?
            .load_from_file_descriptor(read_stdin_impl, supergraph_yaml.as_ref())?;
        eprintln!("supergraph config loaded successfully");
        Ok(CompositionPipeline {
            state: state::ResolveFederationVersion {
                resolver,
                supergraph_root,
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
        prompt: Option<&impl Prompt>,
    ) -> CompositionPipeline<state::InstallSupergraph> {
        let (fed_two_subgraphs, resolved_fed_version) = match self
            .state
            .resolver
            .fully_resolve_subgraphs(
                resolve_introspect_subgraph_factory.clone(),
                fetch_remote_subgraph_factory.clone(),
                &self.state.supergraph_root,
                prompt,
            )
            .await
        {
            Ok((fully_resolved_supergraph_config, errors)) => {
                if errors.is_empty() {
                    let fed_two_subgraphs = fully_resolved_supergraph_config
                        .subgraphs()
                        .iter()
                        .filter_map(|(name, subgraph)| {
                            if subgraph.is_fed_two {
                                Some(name.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<String>>();
                    (
                        fed_two_subgraphs,
                        Some(
                            fully_resolved_supergraph_config
                                .federation_version()
                                .clone(),
                        ),
                    )
                } else {
                    (vec![], None)
                }
            }
            Err(err) => {
                warn!("Could not fully resolve subgraphs: {}", err);
                (vec![], None)
            }
        };
        let federation_version = match (resolved_fed_version, passed_in_fed_version) {
            (None, None) => {
                info!(
                    "No federation version found or supplied, defaulting to {}",
                    LatestFedTwo.get_exact().unwrap().to_string()
                );
                LatestFedTwo
            }
            (Some(resolved_federation_version), None) => resolved_federation_version,
            (_, Some(passed_in_federation_version)) => passed_in_federation_version,
        };

        let federation_version = if !fed_two_subgraphs.is_empty() && federation_version.is_fed_one()
        {
            warn!("Federation version 1, cannot be used with Federation 2 subgraphs");
            warn!("Defaulting to Federation: {}", LatestFedTwo);
            LatestFedTwo
        } else {
            federation_version
        };

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
                .await?;

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
        read_file_impl: &impl ReadFile,
        write_file_impl: &impl WriteFile,
        output_file: Option<Utf8PathBuf>,
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
                None::<&SubgraphPrompt>,
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
                serde_yaml::to_string(&SupergraphConfig::from(fully_resolved_supergraph_config))?
                    .as_bytes(),
            )
            .await
            .map_err(|err| CompositionError::WriteFile {
                path: supergraph_config_filepath.clone(),
                error: Box::new(err),
            })?;

        let result = self
            .state
            .supergraph_binary
            .compose(
                exec_command_impl,
                read_file_impl,
                &output_file
                    .map(OutputTarget::File)
                    .unwrap_or(OutputTarget::Stdout),
                supergraph_config_filepath,
            )
            .await?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn runner<ExecC, ReadF, WriteF>(
        &self,
        exec_command: ExecC,
        read_file: ReadF,
        write_file: WriteF,
        http_service: HttpService,
        make_fetch_remote_subgraph: FetchRemoteSubgraphFactory,
        introspection_polling_interval: u64,
        output_dir: Utf8PathBuf,
        output_target: OutputTarget,
        compose_on_initialisation: bool,
        federation_updater_config: Option<FederationUpdaterConfig>,
        prompt: Option<&SubgraphPrompt>,
    ) -> Result<CompositionRunner<ExecC, ReadF, WriteF>, CompositionPipelineError>
    where
        ReadF: ReadFile + Debug + Eq + PartialEq + Send + Sync + 'static,
        ExecC: ExecCommand + Debug + Eq + PartialEq + Send + Sync + 'static,
        WriteF: WriteFile + Debug + Eq + PartialEq + Send + Sync + 'static,
    {
        // We want to filter down the subgraphs we have at this point,
        // so we want to lazily resolve, and track any subgraphs that won't do that
        // followed by fully resolving and then tracking any subgraphs that won't do that either.
        //
        // The set of subgraphs that will fully resolve, will form our initial set, and
        // then we can return a stream that's been set up as best as possible, with an early stream
        // of errors we can work with (this needs a bit more thought).
        let (
            lazily_resolved_supergraph_config,
            fully_resolved_supergraph_config,
            resolution_errors,
        ) = self
            .generate_lazy_and_fully_resolved_supergraph_configs(prompt)
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
            .setup_supergraph_config_watcher(lazily_resolved_supergraph_config)
            .setup_composition_watcher(
                fully_resolved_supergraph_config,
                resolution_errors,
                self.state.supergraph_binary.clone(),
                exec_command,
                read_file,
                write_file,
                output_dir,
                compose_on_initialisation,
                output_target,
                federation_updater_config,
            );
        Ok(runner)
    }

    async fn generate_lazy_and_fully_resolved_supergraph_configs(
        &self,
        prompt: Option<&SubgraphPrompt>,
    ) -> Result<
        (
            LazilyResolvedSupergraphConfig,
            FullyResolvedSupergraphConfig,
            BTreeMap<String, ResolveSubgraphError>,
        ),
        CompositionPipelineError,
    > {
        // Get the two different kinds of resolutions (we know that the fully_resolved will be a non-proper subset of the lazily_resolved)
        let (lazily_resolved_supergraph_config, lazy_resolution_errors) = self
            .state
            .resolver
            .lazily_resolve_subgraphs(&self.state.supergraph_root, prompt)
            .await?;
        let (fully_resolved_supergraph_config, full_resolution_errors) = self
            .state
            .resolver
            .fully_resolve_subgraphs(
                self.state.resolve_introspect_subgraph_factory.clone(),
                self.state.fetch_remote_subgraph_factory.clone(),
                &self.state.supergraph_root,
                prompt,
            )
            .await?;

        // Generate the correct lazily_resolved config, by removing all the things that cannot fully resolve

        let final_subgraphs = lazily_resolved_supergraph_config
            .subgraphs()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .filter(|(name, _)| full_resolution_errors.contains_key(name))
            .collect();
        let final_lazily_resolved_supergraph_config = LazilyResolvedSupergraphConfig::new(
            lazily_resolved_supergraph_config.origin_path().clone(),
            final_subgraphs,
            lazily_resolved_supergraph_config
                .federation_version()
                .clone(),
        );

        // Merge all the errors together and give all three back
        Ok((
            final_lazily_resolved_supergraph_config,
            fully_resolved_supergraph_config,
            lazy_resolution_errors
                .into_iter()
                .chain(full_resolution_errors)
                .collect(),
        ))
    }
}

mod state {
    use apollo_federation_types::config::FederationVersion;
    use camino::Utf8PathBuf;

    use crate::composition::supergraph::config::full::introspect::ResolveIntrospectSubgraphFactory;
    use crate::composition::supergraph::config::resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory;
    use crate::composition::supergraph::{
        binary::SupergraphBinary, config::resolver::InitializedSupergraphConfigResolver,
    };

    pub struct Init;
    pub struct ResolveFederationVersion {
        pub resolver: InitializedSupergraphConfigResolver,
        pub supergraph_root: Utf8PathBuf,
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
        pub supergraph_binary: SupergraphBinary,
        pub resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        pub fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
    }
}
