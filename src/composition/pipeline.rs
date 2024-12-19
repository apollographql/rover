use std::{collections::BTreeMap, env::current_dir, fmt::Debug, fs::canonicalize};

use apollo_federation_types::config::{FederationVersion, SubgraphConfig, SupergraphConfig};
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use tempfile::tempdir;
use tower::MakeService;

use crate::{
    options::{LicenseAccepter, ProfileOpt},
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::ExecCommand, install::InstallBinary, introspect::IntrospectSubgraph,
            read_file::ReadFile, read_stdin::ReadStdin, write_file::WriteFile,
        },
        parsers::FileDescriptorType,
    },
};

use super::{
    runner::{CompositionRunner, Runner},
    supergraph::{
        binary::OutputTarget,
        config::resolver::{
            fetch_remote_subgraph::{FetchRemoteSubgraphRequest, RemoteSubgraph},
            fetch_remote_subgraphs::FetchRemoteSubgraphsRequest,
            LoadRemoteSubgraphsError, LoadSupergraphConfigError, ResolveSupergraphConfigError,
            SubgraphPrompt, SupergraphConfigResolver,
        },
        install::{InstallSupergraph, InstallSupergraphError},
    },
    CompositionError, CompositionSuccess,
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
}

pub struct CompositionPipeline<State> {
    state: State,
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
        let supergraph_root = supergraph_yaml.clone().and_then(|file| match file {
            FileDescriptorType::File(file) => {
                let mut current_dir = current_dir().expect("Unable to get current directory path");

                current_dir.push(file);
                let path = Utf8PathBuf::from_path_buf(current_dir).unwrap();
                let parent = path.parent().unwrap().to_path_buf();
                Some(parent)
            }
            FileDescriptorType::Stdin => None,
        });
        let resolver = SupergraphConfigResolver::default()
            .load_remote_subgraphs(fetch_remote_subgraphs_factory, graph_ref.as_ref())
            .await?
            .load_from_file_descriptor(read_stdin_impl, supergraph_yaml.as_ref())?;
        Ok(CompositionPipeline {
            state: state::ResolveFederationVersion {
                resolver,
                supergraph_root,
            },
        })
    }
}

impl CompositionPipeline<state::ResolveFederationVersion> {
    pub async fn resolve_federation_version<MakeFetchSubgraph>(
        self,
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: MakeFetchSubgraph,
        federation_version: Option<FederationVersion>,
    ) -> Result<CompositionPipeline<state::InstallSupergraph>, CompositionPipelineError>
    where
        MakeFetchSubgraph:
            MakeService<(), FetchRemoteSubgraphRequest, Response = RemoteSubgraph> + Clone,
        MakeFetchSubgraph::MakeError: std::error::Error + Send + Sync + 'static,
        MakeFetchSubgraph::Error: std::error::Error + Send + Sync + 'static,
    {
        let fully_resolved_supergraph_config = self
            .state
            .resolver
            .fully_resolve_subgraphs(
                introspect_subgraph_impl,
                fetch_remote_subgraph_impl,
                self.state.supergraph_root.as_ref(),
                &SubgraphPrompt::default(),
            )
            .await?;
        let federation_version = federation_version.unwrap_or_else(|| {
            fully_resolved_supergraph_config
                .federation_version()
                .clone()
        });
        Ok(CompositionPipeline {
            state: state::InstallSupergraph {
                resolver: self.state.resolver,
                supergraph_root: self.state.supergraph_root,
                fully_resolved_supergraph_config,
                federation_version,
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
                .await?;

        Ok(CompositionPipeline {
            state: state::Run {
                resolver: self.state.resolver,
                supergraph_root: self.state.supergraph_root,
                fully_resolved_supergraph_config: self.state.fully_resolved_supergraph_config,
                supergraph_binary,
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
        write_file_impl
            .write_file(
                &supergraph_config_filepath,
                serde_yaml::to_string(&SupergraphConfig::from(
                    self.state.fully_resolved_supergraph_config.clone(),
                ))?
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
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
        introspection_polling_interval: u64,
        output_dir: Utf8PathBuf,
    ) -> Result<CompositionRunner<ExecC, ReadF, WriteF>, CompositionPipelineError>
    where
        ReadF: ReadFile + Debug + Eq + PartialEq + Send + Sync + 'static,
        ExecC: ExecCommand + Debug + Eq + PartialEq + Send + Sync + 'static,
        WriteF: WriteFile + Debug + Eq + PartialEq + Send + Sync + 'static,
    {
        let lazily_resolved_supergraph_config = self
            .state
            .resolver
            .lazily_resolve_subgraphs(
                self.state.supergraph_root.as_ref(),
                &SubgraphPrompt::default(),
            )
            .await?;
        let subgraphs = lazily_resolved_supergraph_config.subgraphs().clone();
        let runner = Runner::default()
            .setup_subgraph_watchers(
                subgraphs,
                profile,
                client_config,
                introspection_polling_interval,
            )
            .setup_supergraph_config_watcher(lazily_resolved_supergraph_config)
            .setup_composition_watcher(
                self.state.fully_resolved_supergraph_config.clone(),
                self.state.supergraph_binary.clone(),
                exec_command,
                read_file,
                write_file,
                output_dir,
            );
        Ok(runner)
    }
}

mod state {
    use apollo_federation_types::config::FederationVersion;
    use camino::Utf8PathBuf;

    use crate::composition::supergraph::{
        binary::SupergraphBinary,
        config::{
            full::FullyResolvedSupergraphConfig, resolver::InitializedSupergraphConfigResolver,
        },
    };

    pub struct Init;
    pub struct ResolveFederationVersion {
        pub resolver: InitializedSupergraphConfigResolver,
        pub supergraph_root: Option<Utf8PathBuf>,
    }
    pub struct InstallSupergraph {
        pub resolver: InitializedSupergraphConfigResolver,
        pub supergraph_root: Option<Utf8PathBuf>,
        pub fully_resolved_supergraph_config: FullyResolvedSupergraphConfig,
        pub federation_version: FederationVersion,
    }
    pub struct Run {
        pub resolver: InitializedSupergraphConfigResolver,
        pub supergraph_root: Option<Utf8PathBuf>,
        pub fully_resolved_supergraph_config: FullyResolvedSupergraphConfig,
        pub supergraph_binary: SupergraphBinary,
    }
}
