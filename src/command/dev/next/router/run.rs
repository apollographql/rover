use std::time::Duration;

use anyhow::anyhow;
use apollo_federation_types::config::RouterVersion;
use camino::{Utf8Path, Utf8PathBuf};
use futures::StreamExt;
use houston::Credential;
use rover_client::{
    operations::config::who_am_i::{RegistryIdentity, WhoAmIError, WhoAmIRequest},
    shared::GraphRef,
};
use rover_std::RoverStdError;
use tokio::{
    process::Child,
    time::{sleep, Instant},
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower::Service;
use tracing::{info, warn};

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
    pub async fn run<Spawn>(
        self,
        spawn: Spawn,
        temp_router_dir: &Utf8Path,
        studio_client_config: StudioClientConfig,
    ) -> Result<RunRouter<state::Watch>, RunRouterBinaryError>
    where
        Spawn: Service<ExecCommandConfig, Response = Child> + Send + Clone + 'static,
        Spawn::Error: std::error::Error + Send + Sync,
        Spawn::Future: Send,
    {
        // TODO: make this arguments rather than pulled from the temp-router-dir argument
        let config_path = temp_router_dir.join("config.yaml");
        let schema_path = temp_router_dir.join("supergraph.graphql");

        let run_router_binary = RunRouterBinary::builder()
            .router_binary(self.state.binary.clone())
            .config_path(config_path.clone())
            .supergraph_schema_path(schema_path.clone())
            .and_remote_config(self.state.remote_config.clone())
            .spawn(spawn)
            .build();

        let (_router_log_events, run_router_binary_subtask): (
            UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>>,
            _,
        ) = Subtask::new(run_router_binary);

        let abort_router = SubtaskRunUnit::run(run_router_binary_subtask);

        self.wait_for_healthy_router(&studio_client_config).await?;

        Ok(RunRouter {
            state: state::Watch {
                abort_router,
                config_path,
                schema_path,
            },
        })
    }

    async fn wait_for_healthy_router(
        self,
        studio_client_config: &StudioClientConfig,
    ) -> Result<(), RunRouterBinaryError> {
        if !self.state.config.health_check_enabled() {
            info!("Router healthcheck disabled in the router's configuration. The router might emit errors when starting up, potentially failing to start.");
            return Ok(());
        }

        let healthcheck_endpoint = self.state.config.health_check_endpoint();

        let healthcheck_client = studio_client_config.get_reqwest_client().map_err(|err| {
            RunRouterBinaryError::Internal {
                dependency: "Reqwest Client".to_string(),
                error: format!("Failed to get client: {err}"),
            }
        })?;

        let healthcheck_request = healthcheck_client
            .get(healthcheck_endpoint.to_string())
            .build()
            .map_err(|err| RunRouterBinaryError::Internal {
                dependency: "Reqwest Client".to_string(),
                error: format!("Failed to build healthcheck request: {err}"),
            })?;

        // Wait for the router to become healthy before continuing by checking its health endpoint,
        // waiting only 10s
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut success = false;
            while !success {
                sleep(Duration::from_millis(100)).await;

                let Some(request) = healthcheck_request.try_clone() else {
                    return Err(RunRouterBinaryError::Internal {
                        dependency: "Reqwest Client".to_string(),
                        error: "Failed to clone healthcheck request".to_string(),
                    });
                };

                if let Ok(res) = healthcheck_client.execute(request).await {
                    success = res.status().is_success()
                }
            }
            Ok(())
        })
        .await
        .map_err(|_err| RunRouterBinaryError::HealthCheckFailed)?
    }
}

impl RunRouter<state::Watch> {
    pub async fn watch_for_changes<WriteF>(self, write_file_impl: WriteF) -> RunRouter<state::Abort>
    where
        WriteF: WriteFile + Send + Clone + 'static,
    {
        let config_watcher =
            RouterConfigWatcher::new(FileWatcher::new(self.state.config_path.clone()));
        let (router_config_updates, config_watcher_subtask): (
            UnboundedReceiverStream<RouterUpdateEvent>,
            _,
        ) = Subtask::new(config_watcher);

        let hot_reload_watcher = HotReloadWatcher::builder()
            .config(self.state.config_path)
            .schema(self.state.schema_path)
            .write_file_impl(write_file_impl)
            .build();

        let (_hot_reload_events, hot_reload_subtask): (UnboundedReceiverStream<HotReloadEvent>, _) =
            Subtask::new(hot_reload_watcher);

        let abort_hot_reload =
            SubtaskRunStream::run(hot_reload_subtask, router_config_updates.boxed());

        let abort_config_watcher = SubtaskRunUnit::run(config_watcher_subtask);

        RunRouter {
            state: state::Abort {
                abort_router: self.state.abort_router,
                abort_hot_reload,
                abort_config_watcher,
            },
        }
    }
}

mod state {
    use camino::Utf8PathBuf;
    use tokio::task::AbortHandle;

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
    pub struct Watch {
        pub abort_router: AbortHandle,
        pub config_path: Utf8PathBuf,
        pub schema_path: Utf8PathBuf,
    }
    pub struct Abort {
        pub abort_router: AbortHandle,
        pub abort_config_watcher: AbortHandle,
        pub abort_hot_reload: AbortHandle,
    }
}
