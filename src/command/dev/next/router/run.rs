use apollo_federation_types::config::RouterVersion;
use camino::{Utf8Path, Utf8PathBuf};
use futures::StreamExt;
use houston::Credential;
use rover_client::{
    operations::config::who_am_i::{RegistryIdentity, WhoAmIError, WhoAmIRequest},
    shared::GraphRef,
};
use rover_std::RoverStdError;
use tokio::process::Child;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower::Service;

use crate::{
    command::dev::next::FileWatcher,
    options::LicenseAccepter,
    subtask::{Subtask, SubtaskRunStream, SubtaskRunUnit},
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::ExecCommandConfig, install::InstallBinary, read_file::ReadFile,
            write_file::WriteFile,
        },
    },
};

use super::{
    binary::{RouterLog, RunRouterBinary, RunRouterBinaryError},
    config::{remote::RemoteRouterConfig, ReadRouterConfigError, RouterAddress, RunRouterConfig},
    hot_reload::{HotReloadEvent, HotReloadWatcher, RouterUpdateEvent},
    install::{InstallRouter, InstallRouterError},
    watchers::router_config::RouterConfigWatcher,
};

pub struct RunRouter<S> {
    state: S,
}

impl Default for RunRouter<state::Install> {
    fn default() -> Self {
        RunRouter {
            state: state::Install::default(),
        }
    }
}

impl RunRouter<state::Install> {
    pub async fn install<I: InstallBinary>(
        self,
        router_version: RouterVersion,
        studio_client_config: StudioClientConfig,
        override_install_path: Option<Utf8PathBuf>,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
    ) -> Result<RunRouter<state::LoadLocalConfig>, InstallRouterError> {
        let install_binary = InstallRouter::new(router_version, studio_client_config);
        let binary = install_binary
            .install(override_install_path, elv2_license_accepter, skip_update)
            .await?;
        Ok(RunRouter {
            state: state::LoadLocalConfig { binary },
        })
    }
}

impl RunRouter<state::LoadLocalConfig> {
    pub async fn load_config<ReadF>(
        self,
        read_file_impl: &ReadF,
        router_address: RouterAddress,
        config_path: Utf8PathBuf,
    ) -> Result<RunRouter<state::LoadRemoteConfig>, ReadRouterConfigError>
    where
        ReadF: ReadFile<Error = RoverStdError>,
    {
        let config = RunRouterConfig::default()
            .with_address(router_address)
            .with_config(read_file_impl, &config_path)
            .await?;
        Ok(RunRouter {
            state: state::LoadRemoteConfig {
                binary: self.state.binary,
                config,
                config_path,
            },
        })
    }
}

impl RunRouter<state::LoadRemoteConfig> {
    pub async fn load_remote_config<S>(
        self,
        who_am_i: S,
        graph_ref: Option<GraphRef>,
        credential: Option<Credential>,
    ) -> RunRouter<state::Run>
    where
        S: Service<WhoAmIRequest, Response = RegistryIdentity, Error = WhoAmIError>,
    {
        let state = match graph_ref {
            Some(graph_ref) => {
                let remote_config =
                    RemoteRouterConfig::load(who_am_i, graph_ref.clone(), credential).await;
                state::Run {
                    binary: self.state.binary,
                    config: self.state.config,
                    config_path: self.state.config_path,
                    remote_config: Some(remote_config),
                }
            }
            None => state::Run {
                binary: self.state.binary,
                config: self.state.config,
                config_path: self.state.config_path,
                remote_config: None,
            },
        };
        RunRouter { state }
    }
}

impl RunRouter<state::Run> {
    pub async fn watch_for_changes<WriteF, Spawn>(
        self,
        write_file_impl: WriteF,
        spawn: Spawn,
        temp_router_dir: &Utf8Path,
    ) where
        WriteF: WriteFile + Send + Clone + 'static,
        Spawn: Service<ExecCommandConfig, Response = Child> + Send + Clone + 'static,
        Spawn::Error: std::error::Error + Send + Sync,
        Spawn::Future: Send,
    {
        let config_path = temp_router_dir.join("config.yaml");
        let schema_path = temp_router_dir.join("supergraph.graphql");

        let config_watcher = RouterConfigWatcher::new(FileWatcher::new(self.state.config_path));
        let (router_config_updates, config_watcher_subtask): (
            UnboundedReceiverStream<RouterUpdateEvent>,
            _,
        ) = Subtask::new(config_watcher);

        let hot_reload_watcher = HotReloadWatcher::builder()
            .config(config_path.clone())
            .schema(schema_path.clone())
            .write_file_impl(write_file_impl)
            .build();
        let (_hot_reload_events, hot_reload_subtask): (UnboundedReceiverStream<HotReloadEvent>, _) =
            Subtask::new(hot_reload_watcher);
        let run_router_binary = RunRouterBinary::builder()
            .router_binary(self.state.binary)
            .config_path(config_path)
            .supergraph_schema_path(schema_path)
            .and_remote_config(self.state.remote_config)
            .spawn(spawn)
            .build();
        let (_router_log_events, run_router_binary_subtask): (
            UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>>,
            _,
        ) = Subtask::new(run_router_binary);
        let _abort_router = SubtaskRunUnit::run(run_router_binary_subtask);
        let _abort_hot_reload =
            SubtaskRunStream::run(hot_reload_subtask, router_config_updates.boxed());
        let _abort_config_watcher = SubtaskRunUnit::run(config_watcher_subtask);
    }
}

mod state {
    use camino::Utf8PathBuf;

    use crate::command::dev::next::router::{
        binary::RouterBinary,
        config::{remote::RemoteRouterConfig, RouterConfigFinal},
    };

    #[derive(Default)]
    pub struct Install {}
    pub struct LoadLocalConfig {
        pub binary: RouterBinary,
    }
    pub struct LoadRemoteConfig {
        pub binary: RouterBinary,
        pub config: RouterConfigFinal,
        pub config_path: Utf8PathBuf,
    }
    pub struct Run {
        pub binary: RouterBinary,
        pub config: RouterConfigFinal,
        pub config_path: Utf8PathBuf,
        pub remote_config: Option<RemoteRouterConfig>,
    }
}
