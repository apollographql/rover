use std::{fmt::Display, net::SocketAddr, time::Duration};

use apollo_federation_types::config::RouterVersion;
use camino::{Utf8Path, Utf8PathBuf};
use futures::{
    stream::{self, BoxStream},
    StreamExt,
};
use houston::Credential;
use rover_client::{
    operations::config::who_am_i::{RegistryIdentity, WhoAmIError, WhoAmIRequest},
    shared::GraphRef,
};
use rover_std::{debugln, infoln, RoverStdError};
use tokio::{process::Child, time::sleep};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower::{Service, ServiceExt};

use super::{
    binary::{RouterLog, RunRouterBinary, RunRouterBinaryError},
    config::{remote::RemoteRouterConfig, ReadRouterConfigError, RouterAddress, RunRouterConfig},
    hot_reload::{HotReloadEvent, HotReloadWatcher, RouterUpdateEvent},
    install::{InstallRouter, InstallRouterError},
    watchers::router_config::RouterConfigWatcher,
};
use crate::{
    command::dev::next::{
        router::hot_reload::{self, HotReloadConfig, HotReloadConfigOverrides},
        FileWatcher,
    },
    composition::events::CompositionEvent,
    options::LicenseAccepter,
    subtask::{Subtask, SubtaskRunStream, SubtaskRunUnit},
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::ExecCommandConfig,
            install::InstallBinary,
            read_file::ReadFile,
            write_file::{WriteFile, WriteFileRequest},
        },
    },
};

pub struct RunRouter<S> {
    pub(crate) state: S,
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
        config_path: Option<Utf8PathBuf>,
    ) -> Result<RunRouter<state::LoadRemoteConfig>, ReadRouterConfigError>
    where
        ReadF: ReadFile<Error = RoverStdError>,
    {
        let config = RunRouterConfig::default()
            .with_address(router_address)
            .with_config(read_file_impl, config_path.as_ref())
            .await?;
        if let Some(config_path) = config_path.clone() {
            // TODO: figure out wtf this log means
            infoln!(
                "IS THIS TRUE?! :: Watching {} for changes",
                config_path.as_std_path().display()
            );
        }
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
    pub async fn run<Spawn, WriteFile>(
        self,
        mut write_file: WriteFile,
        spawn: Spawn,
        temp_router_dir: &Utf8Path,
        studio_client_config: StudioClientConfig,
        supergraph_schema: &str,
        credential: Credential,
    ) -> Result<RunRouter<state::Watch>, RunRouterBinaryError>
    where
        Spawn: Service<ExecCommandConfig, Response = Child> + Send + Clone + 'static,
        Spawn::Error: std::error::Error + Send + Sync,
        Spawn::Future: Send,
        WriteFile: Service<WriteFileRequest, Response = ()> + Send + Clone + 'static,
        WriteFile::Error: std::error::Error + Send + Sync,
        WriteFile::Future: Send,
    {
        let write_file = write_file
            .ready()
            .await
            .map_err(|err| RunRouterBinaryError::ServiceReadyError { err: Box::new(err) })?;
        let hot_reload_config_path = temp_router_dir.join("config.yaml");
        tracing::debug!(
            "Creating temporary router config path at {}",
            hot_reload_config_path
        );

        let hot_reload_config = HotReloadConfig::builder()
            .content(self.state.config.raw_config())
            .overrides(
                HotReloadConfigOverrides::builder()
                    // TODO: fix this address
                    .address(self.state.config.address())
                    .build(),
            )
            .build()
            .to_string();

        tracing::debug!("hot reload config: {hot_reload_config:?}");

        write_file
            .call(
                WriteFileRequest::builder()
                    .path(hot_reload_config_path.clone())
                    .contents(Vec::from(hot_reload_config.to_string()))
                    .build(),
            )
            .await
            .map_err(|err| RunRouterBinaryError::WriteFileError {
                path: hot_reload_config_path.clone(),
                err: Box::new(err),
            })?;

        let hot_reload_schema_path = temp_router_dir.join("supergraph.graphql");
        tracing::debug!(
            "Creating temporary supergraph schema path at {}",
            hot_reload_schema_path
        );
        write_file
            .call(
                WriteFileRequest::builder()
                    .path(hot_reload_schema_path.clone())
                    .contents(supergraph_schema.as_bytes().to_vec())
                    .build(),
            )
            .await
            .map_err(|err| RunRouterBinaryError::WriteFileError {
                path: hot_reload_schema_path.clone(),
                err: Box::new(err),
            })?;

        let run_router_binary = RunRouterBinary::builder()
            .router_binary(self.state.binary.clone())
            .config_path(hot_reload_config_path.clone())
            .supergraph_schema_path(hot_reload_schema_path.clone())
            .and_remote_config(self.state.remote_config.clone())
            .credential(credential)
            .spawn(spawn)
            .build();

        let (router_logs, run_router_binary_subtask): (
            UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>>,
            _,
        ) = Subtask::new(run_router_binary);

        let abort_router = SubtaskRunUnit::run(run_router_binary_subtask);

        let mut health_check_endpoint = self
            .state
            .config
            .health_check_endpoint()
            .unwrap()
            .to_string();
        let health_check_path = self.state.config.health_check_path();

        wait_for_healthy_router(
            &mut health_check_endpoint,
            &health_check_path,
            &studio_client_config,
        )
        .await?;

        Ok(RunRouter {
            state: state::Watch {
                abort_router,
                config_path: self.state.config_path,
                hot_reload_config_path,
                hot_reload_schema_path,
                router_logs,
                studio_client_config,
                health_check_endpoint,
                health_check_path,
            },
        })
    }
}

impl RunRouter<state::Watch> {
    pub async fn watch_for_changes<WriteF>(
        self,
        write_file_impl: WriteF,
        composition_messages: BoxStream<'static, CompositionEvent>,
        hot_reload_overrides: HotReloadConfigOverrides,
        studio_client_config: StudioClientConfig,
    ) -> Result<RunRouter<state::Abort>, RunRouterBinaryError>
    where
        WriteF: WriteFile + Send + Clone + 'static,
    {
        tracing::info!("Watching for subgraph changes");
        tracing::debug!("config path in watching: {:?}", self.state.config_path);
        let (router_config_updates, config_watcher_subtask) = if let Some(config_path) =
            self.state.config_path
        {
            let config_watcher = RouterConfigWatcher::new(FileWatcher::new(config_path.clone()));
            let (events, abort_handle): (UnboundedReceiverStream<RouterUpdateEvent>, _) =
                Subtask::new(config_watcher);
            (Some(events), Some(abort_handle))
        } else {
            (None, None)
        };

        tracing::debug!("before composition messages");
        let composition_messages =
            tokio_stream::StreamExt::filter_map(composition_messages, |event| match event {
                CompositionEvent::Started => None,
                CompositionEvent::Error(err) => {
                    tracing::error!("Composition error {:?}", err);
                    None
                }
                CompositionEvent::Success(success) => Some(RouterUpdateEvent::SchemaChanged {
                    schema: success.supergraph_sdl().to_string(),
                }),
            })
            .boxed();
        tracing::debug!("after composition messages");

        tracing::debug!("before hot reload watchier");
        let hot_reload_watcher = HotReloadWatcher::builder()
            .config(self.state.hot_reload_config_path)
            .schema(self.state.hot_reload_schema_path.clone())
            .overrides(hot_reload_overrides)
            .write_file_impl(write_file_impl)
            .build();
        tracing::debug!("after hot reload watchier");

        tracing::debug!("before subtask for hot reload watcher");
        let (hot_reload_events, hot_reload_subtask): (UnboundedReceiverStream<HotReloadEvent>, _) =
            Subtask::new(hot_reload_watcher);
        tracing::debug!("after subtask for hot reload watcher");

        tracing::debug!("before router config updates");
        let router_config_updates = router_config_updates
            .map(move |stream| stream.boxed())
            .unwrap_or_else(|| stream::empty().boxed());
        tracing::debug!("after router config updates");

        tracing::debug!("before router updates merge");
        let router_updates =
            tokio_stream::StreamExt::merge(router_config_updates, composition_messages);
        tracing::debug!("after router updates merge");

        tracing::debug!("before abort handles");
        let abort_hot_reload = SubtaskRunStream::run(hot_reload_subtask, router_updates.boxed());

        let abort_config_watcher = config_watcher_subtask.map(SubtaskRunUnit::run);
        tracing::debug!("after abort handles");

        let mut endpoint = self.state.health_check_endpoint;
        if let Err(_whoopsie) = wait_for_healthy_router(
            &mut endpoint,
            &self.state.health_check_path,
            &studio_client_config,
        )
        .await
        {
            // FIXME: doesn't actually abort!
            tracing::debug!("aborting!");
            abort_config_watcher.clone().unwrap().abort();
            abort_hot_reload.abort();
            self.state.abort_router.abort();
        };

        Ok(RunRouter {
            state: state::Abort {
                abort_router: self.state.abort_router,
                abort_hot_reload,
                abort_config_watcher,
                hot_reload_events,
                router_logs: self.state.router_logs,
                hot_reload_schema_path: self.state.hot_reload_schema_path,
            },
        })
    }
}

impl RunRouter<state::Abort> {
    pub fn router_logs(
        &mut self,
    ) -> &mut UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>> {
        &mut self.state.router_logs
    }

    pub fn shutdown(&mut self) {
        self.state.abort_router.abort();
        self.state.abort_hot_reload.abort();
        if let Some(abort) = self.state.abort_config_watcher.take() {
            abort.abort();
        };
    }
}

async fn wait_for_healthy_router(
    health_check_endpoint: &mut String,
    health_check_path: &String,
    studio_client_config: &StudioClientConfig,
) -> Result<(), RunRouterBinaryError> {
    //if !self.state.config.health_check_enabled() {
    //    info!("Router healthcheck disabled in the router's configuration. The router might emit errors when starting up, potentially failing to start.");
    //    return Ok(());
    //}

    // We hardcode the endpoint and port; if they're missing now, we've lost that bit of code
    //let mut healthcheck_endpoint = match health_check_endpoint {
    //        Some(endpoint) => endpoint.to_string(),
    //        None => {
    //        return Err(RunRouterBinaryError::Internal {
    //            dependency: "Router Config Validation".to_string(),
    //            err: format!("Router Config passed validation incorrectly, healthchecks are enabled but missing an endpoint"),
    //        })
    //        }
    //    };

    health_check_endpoint.push_str(&health_check_path);
    let healthcheck_client = studio_client_config.get_reqwest_client().map_err(|err| {
        RunRouterBinaryError::Internal {
            dependency: "Reqwest Client".to_string(),
            err: format!("Failed to get client: {err}"),
        }
    })?;

    let healthcheck_request = healthcheck_client
        .get(format!("http://{health_check_endpoint}"))
        .build()
        .map_err(|err| RunRouterBinaryError::Internal {
            dependency: "Reqwest Client".to_string(),
            err: format!("Failed to build healthcheck request: {err}"),
        })?;

    // Wait for the router to become healthy before continuing by checking its health endpoint,
    // waiting only 10s
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut success = false;
        while !success {
            sleep(Duration::from_millis(250)).await;

            let Some(request) = healthcheck_request.try_clone() else {
                return Err(RunRouterBinaryError::Internal {
                    dependency: "Reqwest Client".to_string(),
                    err: "Failed to clone healthcheck request".to_string(),
                });
            };

            tracing::debug!("sending health check ping to the router process");
            debugln!("sending router health check");

            if let Ok(res) = healthcheck_client.execute(request).await {
                success = res.status().is_success();
                if success {
                    tracing::debug!("health check successful!");
                    debugln!("health check successful!");
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|_err| {
        tracing::error!("health check failed");
        RunRouterBinaryError::HealthCheckFailed
    })?
}

mod state {
    use camino::Utf8PathBuf;
    use tokio::task::AbortHandle;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    use crate::{
        command::dev::next::router::{
            binary::{RouterBinary, RouterLog, RunRouterBinaryError},
            config::{remote::RemoteRouterConfig, RouterConfigFinal},
            hot_reload::HotReloadEvent,
        },
        utils::client::StudioClientConfig,
    };

    #[derive(Default)]
    pub struct Install {}
    pub struct LoadLocalConfig {
        pub binary: RouterBinary,
    }
    pub struct LoadRemoteConfig {
        pub binary: RouterBinary,
        pub config: RouterConfigFinal,
        pub config_path: Option<Utf8PathBuf>,
    }
    pub struct Run {
        pub binary: RouterBinary,
        pub config: RouterConfigFinal,
        pub config_path: Option<Utf8PathBuf>,
        pub remote_config: Option<RemoteRouterConfig>,
    }
    pub struct Watch {
        pub abort_router: AbortHandle,
        pub config_path: Option<Utf8PathBuf>,
        pub hot_reload_config_path: Utf8PathBuf,
        pub hot_reload_schema_path: Utf8PathBuf,
        pub router_logs: UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>>,
        pub health_check_endpoint: String,
        pub health_check_path: String,
        pub studio_client_config: StudioClientConfig,
    }
    pub struct Abort {
        pub router_logs: UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>>,
        #[allow(unused)]
        pub hot_reload_events: UnboundedReceiverStream<HotReloadEvent>,
        #[allow(unused)]
        pub abort_router: AbortHandle,
        #[allow(unused)]
        pub abort_config_watcher: Option<AbortHandle>,
        #[allow(unused)]
        pub abort_hot_reload: AbortHandle,
        #[allow(unused)]
        pub hot_reload_schema_path: Utf8PathBuf,
    }
}
