//! A [`Runner`] provides methods for configuring and handling background tasks for producing
//! composition events based of supergraph config changes.

#![warn(missing_docs)]

use std::{collections::BTreeMap, env::current_dir, fmt::Debug, io::stdin};

//use std::{env::current_dir, fs::File, process::Command, str};

use anyhow::anyhow;
use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::stream::{BoxStream, StreamExt};
use rover_client::shared::GraphRef;
use rover_std::warnln;
use tempfile::tempdir;

use crate::{
    command::supergraph::compose::CompositionOutput,
    composition::{
        supergraph::install::InstallSupergraph,
        watchers::watcher::{file::FileWatcher, supergraph_config::SupergraphConfigWatcher},
    },
    options::{LicenseAccepter, ProfileOpt},
    subtask::{Subtask, SubtaskRunStream, SubtaskRunUnit},
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::{ExecCommand, TokioCommand},
            install::InstallBinary,
            read_file::{FsReadFile, ReadFile},
            write_file::{FsWriteFile, WriteFile},
        },
        parsers::FileDescriptorType,
    },
    RoverError, RoverResult,
};

use self::state::SetupSubgraphWatchers;

use super::{
    events::CompositionEvent,
    supergraph::{
        binary::{OutputTarget, SupergraphBinary},
        config::{
            full::FullyResolvedSubgraphs,
            lazy::{LazilyResolvedSubgraph, LazilyResolvedSupergraphConfig},
            resolver::SupergraphConfigResolver,
        },
    },
    watchers::{composition::CompositionWatcher, subgraphs::SubgraphWatchers},
};

mod state;

/// A struct for configuring and running subtasks for watching for both supergraph and subgraph
/// change events.
/// This is parameterized around the values in the [`state`] module, as to provide
/// a type-based workflow for configuring and running the [`Runner`]
///
/// The configuration flow goes as follows:
/// Runner<SetupSubgraphWatchers>
///   -> Runner<SetupSupergraphConfigWatcher>
///   -> Runner<SetupCompositionWatcher>
///   -> Runner<Run>
// TODO: handle retry flag for subgraphs (see rover dev help)
pub struct Runner<State> {
    state: State,
}

/// Everything necessary to run composition once
#[derive(Builder)]
pub struct OneShotComposition {
    override_install_path: Option<Utf8PathBuf>,
    federation_version: Option<FederationVersion>,
    client_config: StudioClientConfig,
    profile: ProfileOpt,
    supergraph_yaml: Option<FileDescriptorType>,
    output_file: Option<Utf8PathBuf>,
    graph_ref: Option<GraphRef>,
    elv2_license_accepter: LicenseAccepter,
    skip_update: bool,
}

impl OneShotComposition {
    /// Runs composition
    pub async fn compose(self) -> RoverResult<CompositionOutput> {
        let mut stdin = stdin();
        let write_file = FsWriteFile::default();
        let read_file = FsReadFile::default();
        let exec_command = TokioCommand::default();

        let supergraph_root = self.supergraph_yaml.clone().and_then(|file| match file {
            FileDescriptorType::File(file) => {
                let mut current_dir = current_dir().expect("Unable to get current directory path");

                current_dir.push(file);
                let path = Utf8PathBuf::from_path_buf(current_dir).unwrap();
                let parent = path.parent().unwrap().to_path_buf();
                Some(parent)
            }
            FileDescriptorType::Stdin => None,
        });

        let studio_client = self
            .client_config
            .get_authenticated_client(&self.profile.clone())?;

        // Get a FullyResolvedSupergraphConfig from first loading in any remote subgraphs and then
        // a local supergraph config (if present) and then combining them into a fully resolved
        // supergraph config
        let resolver = SupergraphConfigResolver::default()
            .load_remote_subgraphs(&studio_client, self.graph_ref.as_ref())
            .await?
            .load_from_file_descriptor(&mut stdin, self.supergraph_yaml.as_ref())?
            .fully_resolve_subgraphs(
                &self.client_config,
                &studio_client,
                supergraph_root.as_ref(),
            )
            .await?;

        // We convert the FullyResolvedSupergraphConfig into a Supergraph because it makes using
        // Serde easier (said differently: we're using the Federation-rs types here for
        // compatability with Federation-rs tooling later on when we use their supergraph binary to
        // actually run composition)
        let supergraph_config: SupergraphConfig = resolver.clone().into();

        // Convert the FullyResolvedSupergraphConfig to yaml before we save it
        let supergraph_config_yaml = serde_yaml::to_string(&supergraph_config)?;

        // We're going to save to a temporary place because we don't actually need the supergraph
        // config to stick around; we only need it on disk to point the supergraph binary at
        let supergraph_config_filepath =
            Utf8PathBuf::from_path_buf(tempdir()?.path().join("supergraph.yaml"))
                .expect("Unable to parse path");

        // Write the supergraph config to disk
        write_file
            .write_file(
                &supergraph_config_filepath,
                supergraph_config_yaml.as_bytes(),
            )
            .await?;

        // Use the CLI option for federation over the one we can read off of the supergraph config
        // (but default to the one we can read off the supergraph config)
        let fed_version = self
            .federation_version
            .as_ref()
            .unwrap_or(resolver.federation_version());

        // We care about the exact version of the federation version because certain options aren't
        // available before 2.9.0 and we gate on that version below
        let exact_version = fed_version
            .get_exact()
            // This should be impossible to get to because we convert to a FederationVersion a few
            // lines above and so _should_ have an exact version
            .ok_or(RoverError::new(anyhow!(
                "failed to get exact Federation version"
            )))?;

        // Making the output file mutable allows us to change it if we're using a version of the
        // supergraph binary that can't write to file (ie, anything pre-2.9.0)
        let mut output_file = self.output_file;

        // When the `--output` flag is used, we need a supergraph binary version that is at least
        // v2.9.0. We ignore that flag for composition when we have anything less than that
        if output_file.is_some()
            && (exact_version.major < 2 || (exact_version.major == 2 && exact_version.minor < 9))
        {
            warnln!("ignoring `--output` because it is not supported in this version of the dependent binary, `supergraph`: {}. Upgrade to Federation 2.9.0 or greater to install a version of the binary that supports it.", fed_version);
            output_file = None;
        }

        // Build the supergraph binary, paying special attention to the CLI options
        let supergraph_binary =
            InstallSupergraph::new(fed_version.clone(), self.client_config.clone())
                .install(
                    self.override_install_path,
                    self.elv2_license_accepter,
                    self.skip_update,
                )
                .await?;

        let result = supergraph_binary
            .compose(
                &exec_command,
                &read_file,
                &output_file
                    .map(OutputTarget::File)
                    .unwrap_or(OutputTarget::Stdout),
                supergraph_config_filepath,
            )
            .await?;

        Ok(result.into())
    }
}

impl Default for Runner<SetupSubgraphWatchers> {
    fn default() -> Self {
        Runner {
            state: state::SetupSubgraphWatchers,
        }
    }
}

impl Runner<state::SetupSubgraphWatchers> {
    /// Configures the subgraph watchers for the [`Runner`]
    pub fn setup_subgraph_watchers(
        self,
        subgraphs: BTreeMap<String, LazilyResolvedSubgraph>,
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
        introspection_polling_interval: u64,
    ) -> Runner<state::SetupSupergraphConfigWatcher> {
        let subgraph_watchers = SubgraphWatchers::new(
            subgraphs,
            profile,
            client_config,
            introspection_polling_interval,
        );
        Runner {
            state: state::SetupSupergraphConfigWatcher { subgraph_watchers },
        }
    }
}

impl Runner<state::SetupSupergraphConfigWatcher> {
    /// Configures the supergraph watcher for the [`Runner`]
    pub fn setup_supergraph_config_watcher(
        self,
        supergraph_config: LazilyResolvedSupergraphConfig,
    ) -> Runner<state::SetupCompositionWatcher> {
        // If the supergraph config was passed as a file, we can configure a watcher for change
        // events.
        // We could return None here if we received a supergraph config directly from stdin. In
        // that case, we don't want to configure a watcher.
        let supergraph_config_watcher = if let Some(origin_path) = supergraph_config.origin_path() {
            let f = FileWatcher::new(origin_path.clone());
            let watcher = SupergraphConfigWatcher::new(f, supergraph_config);
            Some(watcher)
        } else {
            None
        };
        Runner {
            state: state::SetupCompositionWatcher {
                supergraph_config_watcher,
                subgraph_watchers: self.state.subgraph_watchers,
            },
        }
    }
}

impl Runner<state::SetupCompositionWatcher> {
    /// Configures the composition watcher
    #[allow(clippy::too_many_arguments)]
    pub fn setup_composition_watcher<ReadF, ExecC, WriteF>(
        self,
        subgraphs: FullyResolvedSubgraphs,
        supergraph_binary: SupergraphBinary,
        exec_command: ExecC,
        read_file: ReadF,
        write_file: WriteF,
        output_target: OutputTarget,
        temp_dir: Utf8PathBuf,
    ) -> Runner<state::Run<ReadF, ExecC, WriteF>>
    where
        ReadF: ReadFile + Debug + Eq + PartialEq + Send + Sync + 'static,
        ExecC: ExecCommand + Debug + Eq + PartialEq + Send + Sync + 'static,
        WriteF: WriteFile + Debug + Eq + PartialEq + Send + Sync + 'static,
    {
        // Create a handler for supergraph composition events.
        let composition_watcher = CompositionWatcher::builder()
            .subgraphs(subgraphs)
            .supergraph_binary(supergraph_binary)
            .exec_command(exec_command)
            .read_file(read_file)
            .write_file(write_file)
            .output_target(output_target)
            .temp_dir(temp_dir)
            .build();
        Runner {
            state: state::Run {
                subgraph_watchers: self.state.subgraph_watchers,
                supergraph_config_watcher: self.state.supergraph_config_watcher,
                composition_watcher,
            },
        }
    }
}

impl<ReadF, ExecC, WriteF> Runner<state::Run<ReadF, ExecC, WriteF>>
where
    ReadF: ReadFile + Debug + Eq + PartialEq + Send + Sync + 'static,
    ExecC: ExecCommand + Debug + Eq + PartialEq + Send + Sync + 'static,
    WriteF: WriteFile + Debug + Eq + PartialEq + Send + Sync + 'static,
{
    /// Runs the [`Runner`]
    pub fn run(self) -> BoxStream<'static, CompositionEvent> {
        let (supergraph_config_stream, supergraph_config_subtask) =
            if let Some(supergraph_config_watcher) = self.state.supergraph_config_watcher {
                let (supergraph_config_stream, supergraph_config_subtask) =
                    Subtask::new(supergraph_config_watcher);
                (
                    supergraph_config_stream.boxed(),
                    Some(supergraph_config_subtask),
                )
            } else {
                (tokio_stream::empty().boxed(), None)
            };

        let (subgraph_change_stream, subgraph_watcher_subtask) =
            Subtask::new(self.state.subgraph_watchers);

        // Create a new subtask for the composition handler, passing in a stream of subgraph change
        // events in order to trigger recomposition.
        let (composition_messages, composition_subtask) =
            Subtask::new(self.state.composition_watcher);
        composition_subtask.run(subgraph_change_stream.boxed());

        // Start subgraph watchers, listening for events from the supergraph change stream.
        subgraph_watcher_subtask.run(supergraph_config_stream);

        // Start the supergraph watcher subtask.
        if let Some(supergraph_config_subtask) = supergraph_config_subtask {
            supergraph_config_subtask.run();
        }

        composition_messages.boxed()
    }
}
