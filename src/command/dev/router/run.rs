use std::collections::HashMap;
use std::time::Duration;

use super::binary::{RouterLog, RunRouterBinary, RunRouterBinaryError};
use super::config::remote::RemoteRouterConfig;
use super::config::{ReadRouterConfigError, RouterAddress, RunRouterConfig};
use super::hot_reload::{HotReloadEvent, HotReloadWatcher, RouterUpdateEvent};
use super::install::{InstallRouter, InstallRouterError};
use super::watchers::router_config::RouterConfigWatcher;
use crate::command::dev::router::hot_reload::{HotReloadConfig, HotReloadConfigOverrides};
use crate::command::dev::router::watchers::file::FileWatcher;
use crate::composition::events::CompositionEvent;
use crate::composition::CompositionError;
use crate::options::{LicenseAccepter, ProfileOpt, DEFAULT_PROFILE};
use crate::subtask::{Subtask, SubtaskRunStream, SubtaskRunUnit};
use crate::utils::client::StudioClientConfig;
use crate::utils::effect::exec::ExecCommandConfig;
use crate::utils::effect::install::InstallBinary;
use crate::utils::effect::read_file::ReadFile;
use crate::utils::effect::write_file::{WriteFile, WriteFileRequest};
use crate::RoverError;
use apollo_federation_types::config::RouterVersion;
use camino::{Utf8Path, Utf8PathBuf};
use futures::stream::{self, BoxStream};
use futures::StreamExt;
use houston::{Config, Profile};
use rover_client::shared::GraphRef;
use rover_client::RoverClientError;
use rover_std::{debugln, errln, infoln, warnln, RoverStdError};
use timber::Level;
use tokio::process::Child;
use tokio::time::sleep;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceExt};

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
    pub async fn install(
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
            // This is a somewhat misleading place to alert users that their config is being
            // watched (because there's no watching logic _here_); look at the
            // RunRouter<state::Watch> impl for the actual watching logic
            infoln!(
                "Watching {} for changes",
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
    pub async fn load_remote_config(
        self,
        client_config: StudioClientConfig,
        profile: ProfileOpt,
        graph_ref: Option<GraphRef>,
        home_override: Option<String>,
        api_key_override: Option<String>,
    ) -> RunRouter<state::Run> {
        let state = match graph_ref {
            Some(graph_ref) => {
                let remote_config = RemoteRouterConfig::load(
                    client_config,
                    profile,
                    graph_ref.clone(),
                    home_override,
                    api_key_override,
                )
                .await;
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
    #[allow(clippy::too_many_arguments)]
    pub async fn run<Spawn, WriteFile>(
        self,
        mut write_file: WriteFile,
        spawn: Spawn,
        temp_router_dir: &Utf8Path,
        studio_client_config: StudioClientConfig,
        supergraph_schema: &str,
        profile: ProfileOpt,
        home_override: Option<String>,
        api_key_override: Option<String>,
        log_level: Option<Level>,
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

        let hot_reload_config = HotReloadConfig::new(
            self.state.config.raw_config(),
            Some(
                HotReloadConfigOverrides::builder()
                    .address(*self.state.config.address())
                    .build(),
            ),
        )
        .map_err(RunRouterBinaryError::from)?
        .to_string();

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

        let env = self.auth_env(profile, home_override, api_key_override);
        let run_router_binary = RunRouterBinary::builder()
            .router_binary(self.state.binary.clone())
            .config_path(hot_reload_config_path.clone())
            .supergraph_schema_path(hot_reload_schema_path.clone())
            .env(env.clone())
            .and_log_level(log_level)
            .spawn(spawn)
            .build();

        let (router_logs, run_router_binary_subtask): (
            UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>>,
            _,
        ) = Subtask::new(run_router_binary);

        let cancellation_token = CancellationToken::new();
        SubtaskRunUnit::run(run_router_binary_subtask, Some(cancellation_token.clone()));

        self.wait_for_healthy_router(&studio_client_config).await?;

        Ok(RunRouter {
            state: state::Watch {
                cancellation_token,
                config_path: self.state.config_path,
                hot_reload_config_path,
                hot_reload_schema_path,
                router_logs,
                env,
            },
        })
    }

    fn auth_env(
        &self,
        profile: ProfileOpt,
        home_override: Option<String>,
        api_key_override: Option<String>,
    ) -> HashMap<String, String> {
        let mut env = HashMap::from_iter([("APOLLO_ROVER".to_string(), "true".to_string())]);

        // We set the APOLLO_KEY here, but it might be overridden by RemoteRouterConfig. That
        // struct takes the who_am_i service, gets an identity, and checks whether the
        // associated API key (if present) is of a graph-level actor; if it is, we overwrite
        // the env key with it because we know it's associated with the target graph_ref
        match &self.state.remote_config {
            Some(remote_config) => {
                if let Some(api_key) = remote_config.api_key().clone() {
                    env.insert("APOLLO_KEY".to_string(), api_key);
                }
                env.insert(
                    "APOLLO_GRAPH_REF".to_string(),
                    remote_config.graph_ref().to_string(),
                );
            }
            None => {
                match Config::new(home_override.as_ref(), api_key_override.clone())
                    .and_then(|config| Profile::get_credential(&profile.profile_name, &config))
                {
                    Ok(credential) => {
                        env.insert("APOLLO_KEY".to_string(), credential.api_key.clone());
                    }
                    Err(err) => {
                        if profile.profile_name != DEFAULT_PROFILE {
                            warnln!("Could not retrieve APOLLO_KEY for profile {}.\n{}\nContinuing to load router without an APOLLO_KEY", profile.profile_name, err)
                        }
                    }
                };
            }
        }
        env
    }

    async fn wait_for_healthy_router(
        &self,
        studio_client_config: &StudioClientConfig,
    ) -> Result<(), RunRouterBinaryError> {
        if !self.state.config.health_check_enabled() {
            tracing::info!("Router healthcheck disabled in the router's configuration. The router might emit errors when starting up, potentially failing to start.");
            return Ok(());
        }

        // We hardcode the endpoint and port; if they're missing now, we've lost that bit of code
        let mut healthcheck_endpoint = match self.state.config.health_check_endpoint() {
            Some(endpoint) => endpoint.to_string(),
            None => {
            return Err(RunRouterBinaryError::Internal {
                dependency: "Router Config Validation".to_string(),
                err: String::from("Router Config passed validation incorrectly, healthchecks are enabled but missing an endpoint")
            })
            }
        };

        healthcheck_endpoint.push_str(&self.state.config.health_check_path());
        let healthcheck_client = studio_client_config.get_reqwest_client().map_err(|err| {
            RunRouterBinaryError::Internal {
                dependency: "Reqwest Client".to_string(),
                err: format!("Failed to get client: {err}"),
            }
        })?;

        let healthcheck_request = healthcheck_client
            .get(format!("http://{healthcheck_endpoint}"))
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
}

impl RunRouter<state::Watch> {
    pub async fn watch_for_changes<WriteF>(
        self,
        write_file_impl: WriteF,
        composition_messages: BoxStream<'static, CompositionEvent>,
        hot_reload_overrides: HotReloadConfigOverrides,
    ) -> RunRouter<state::Abort>
    where
        WriteF: WriteFile + Send + Clone + 'static,
    {
        tracing::info!("Watching for subgraph changes");
        let (router_config_updates, config_watcher_subtask) = if let Some(config_path) =
            self.state.config_path
        {
            let config_watcher = RouterConfigWatcher::new(FileWatcher::new(config_path.clone()));
            let (events, subtask): (UnboundedReceiverStream<RouterUpdateEvent>, _) =
                Subtask::new(config_watcher);
            (Some(events), Some(subtask))
        } else {
            (None, None)
        };

        let composition_messages =
            tokio_stream::StreamExt::filter_map(composition_messages, |event| match event {
                CompositionEvent::Error(CompositionError::Build { source, .. }) => {
                    let number_of_subgraphs = source.len();
                    let error_to_output = RoverError::from(RoverClientError::BuildErrors {
                        source,
                        num_subgraphs: number_of_subgraphs,
                    });
                    eprintln!("{error_to_output}");
                    None
                }
                CompositionEvent::Error(err) => {
                    tracing::error!("Composition error {:?}", err);
                    errln!("Error occurred when composing supergraph\n{}", err);
                    None
                }
                CompositionEvent::Success(success) => Some(RouterUpdateEvent::SchemaChanged {
                    schema: success.supergraph_sdl().to_string(),
                }),
                _ => None,
            })
            .boxed();

        let hot_reload_watcher = HotReloadWatcher::builder()
            .config(self.state.hot_reload_config_path)
            .schema(self.state.hot_reload_schema_path.clone())
            .overrides(hot_reload_overrides)
            .write_file_impl(write_file_impl)
            .build();

        let (hot_reload_events, hot_reload_subtask): (UnboundedReceiverStream<HotReloadEvent>, _) =
            Subtask::new(hot_reload_watcher);
        let router_config_updates = router_config_updates
            .map(move |stream| stream.boxed())
            .unwrap_or_else(|| stream::empty().boxed());
        let router_updates =
            tokio_stream::StreamExt::merge(router_config_updates, composition_messages);

        SubtaskRunStream::run(
            hot_reload_subtask,
            router_updates.boxed(),
            Some(self.state.cancellation_token.clone()),
        );

        if let Some(subtask) = config_watcher_subtask {
            subtask.run(Some(self.state.cancellation_token.clone()))
        }

        RunRouter {
            state: state::Abort {
                cancellation_token: self.state.cancellation_token.clone(),
                hot_reload_events,
                router_logs: self.state.router_logs,
                hot_reload_schema_path: self.state.hot_reload_schema_path,
                env: self.state.env,
            },
        }
    }
}

impl RunRouter<state::Abort> {
    pub fn router_logs(
        &mut self,
    ) -> &mut UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>> {
        &mut self.state.router_logs
    }

    pub fn shutdown(&mut self) {
        self.state.cancellation_token.cancel();
    }
}

mod state {
    use std::collections::HashMap;

    use camino::Utf8PathBuf;
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tokio_util::sync::CancellationToken;

    use crate::command::dev::router::binary::{RouterBinary, RouterLog, RunRouterBinaryError};
    use crate::command::dev::router::config::remote::RemoteRouterConfig;
    use crate::command::dev::router::config::RouterConfigFinal;
    use crate::command::dev::router::hot_reload::HotReloadEvent;

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
        pub cancellation_token: CancellationToken,
        pub config_path: Option<Utf8PathBuf>,
        pub hot_reload_config_path: Utf8PathBuf,
        pub hot_reload_schema_path: Utf8PathBuf,
        pub router_logs: UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>>,
        pub env: HashMap<String, String>,
    }
    pub struct Abort {
        pub router_logs: UnboundedReceiverStream<Result<RouterLog, RunRouterBinaryError>>,
        #[allow(unused)]
        pub hot_reload_events: UnboundedReceiverStream<HotReloadEvent>,
        #[allow(unused)]
        pub cancellation_token: CancellationToken,
        #[allow(unused)]
        pub hot_reload_schema_path: Utf8PathBuf,
        pub env: HashMap<String, String>,
    }
}
