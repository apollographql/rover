use std::io::stdin;
use std::str::FromStr;

use anyhow::anyhow;
use apollo_federation_types::config::{FederationVersion, RouterVersion};
use camino::Utf8PathBuf;
use futures::StreamExt;
use houston::{Config, Profile};
use rover_client::operations::config::who_am_i::WhoAmI;
use rover_client::RoverClientError;
use rover_std::{errln, infoln, warnln};
use semver::Version;
use tower::ServiceExt;

use super::version_upgrade_message::VersionUpgradeMessage;
use crate::command::dev::router::binary::RunRouterBinaryError;
use crate::command::dev::router::config::{RouterAddress, RouterHost, RouterPort};
use crate::command::dev::router::hot_reload::HotReloadConfigOverrides;
use crate::command::dev::router::run::RunRouter;
use crate::command::dev::{OVERRIDE_DEV_COMPOSITION_VERSION, OVERRIDE_DEV_ROUTER_VERSION};
use crate::command::Dev;
use crate::composition::events::CompositionEvent;
use crate::composition::pipeline::CompositionPipeline;
use crate::composition::supergraph::config::full::introspect::MakeResolveIntrospectSubgraph;
use crate::composition::supergraph::config::resolver::fetch_remote_subgraph::MakeFetchRemoteSubgraph;
use crate::composition::supergraph::config::resolver::fetch_remote_subgraphs::MakeFetchRemoteSubgraphs;
use crate::composition::supergraph::config::resolver::{DefaultSubgraphDefinition, SubgraphPrompt};
use crate::composition::{CompositionError, FederationUpdaterConfig};
use crate::utils::client::StudioClientConfig;
use crate::utils::effect::exec::{TokioCommand, TokioSpawn};
use crate::utils::effect::read_file::FsReadFile;
use crate::utils::effect::write_file::FsWriteFile;
use crate::utils::env::RoverEnvKey;
use crate::{RoverError, RoverOutput, RoverResult};

impl Dev {
    /// Runs rover dev
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        VersionUpgradeMessage::print();
        warnln!(
            "Do not run this command in production! It is intended for local development only.\n"
        );

        let elv2_license_accepter = self.opts.plugin_opts.elv2_license_accepter;
        let skip_update = self.opts.plugin_opts.skip_update;
        let read_file_impl = FsReadFile::default();
        let write_file_impl = FsWriteFile::default();
        let exec_command_impl = TokioCommand::default();

        let tmp_dir = tempfile::Builder::new().prefix("supergraph").tempdir()?;
        let tmp_config_dir_path = Utf8PathBuf::try_from(tmp_dir.into_path())?;

        let router_config_path = self.opts.supergraph_opts.router_config_path.clone();

        let profile = &self.opts.plugin_opts.profile;
        let graph_ref = &self.opts.supergraph_opts.graph_ref;
        if let Some(graph_ref) = graph_ref {
            eprintln!("retrieving subgraphs remotely from {graph_ref}")
        }
        let supergraph_config_path = &self.opts.supergraph_opts.clone().supergraph_config_path;

        let service = client_config
            .get_authenticated_client(profile)?
            .studio_graphql_service()?;
        let who_am_i_service = WhoAmI::new(service);

        let fetch_remote_subgraphs_factory = MakeFetchRemoteSubgraphs::builder()
            .studio_client_config(client_config.clone())
            .profile(profile.clone())
            .build();
        let fetch_remote_subgraph_factory = MakeFetchRemoteSubgraph::builder()
            .studio_client_config(client_config.clone())
            .profile(profile.clone())
            .build()
            .boxed_clone();
        let resolve_introspect_subgraph_factory =
            MakeResolveIntrospectSubgraph::new(client_config.service()?).boxed_clone();

        // We resolve supergraph binary overrides (ie, composition version) in this order:
        //
        // 1) cli option
        // 2) env var override
        // 3) what's in the supergraph config (represented here as None)
        let federation_version = self
            .opts
            .supergraph_opts
            .federation_version
            .clone()
            .or_else(|| {
                let version = &OVERRIDE_DEV_COMPOSITION_VERSION
                    .clone()
                    .and_then(|version| {
                        match FederationVersion::from_str(&format!("={version}")) {
                            Ok(version) => Some(version),
                            Err(err) => {
                                errln!("{err}");
                                tracing::error!("{:?}", err);
                                None
                            }
                        }
                    });

                version.clone()
            });

        let subgraph_definition = self
            .opts
            .subgraph_opts
            .subgraph_name
            .as_ref()
            .and_then(|subgraph_name| {
                self.opts
                    .subgraph_opts
                    .subgraph_url
                    .as_ref()
                    .map(|subgraph_url| DefaultSubgraphDefinition::Args {
                        name: subgraph_name.to_string(),
                        url: subgraph_url.clone(),
                        schema_path: self.opts.subgraph_opts.subgraph_schema_path.clone(),
                    })
            })
            .unwrap_or_else(|| {
                DefaultSubgraphDefinition::Prompt(Box::new(SubgraphPrompt::default()))
            });
        let composition_pipeline = CompositionPipeline::default()
            .init(
                &mut stdin(),
                fetch_remote_subgraphs_factory,
                supergraph_config_path.clone(),
                graph_ref.clone(),
                Some(subgraph_definition),
            )
            .await?
            .resolve_federation_version(
                resolve_introspect_subgraph_factory.clone(),
                fetch_remote_subgraph_factory.clone(),
                federation_version,
            )
            .await
            .install_supergraph_binary(
                client_config.clone(),
                override_install_path.clone(),
                elv2_license_accepter,
                skip_update,
            )
            .await?;

        let router_version = match &*OVERRIDE_DEV_ROUTER_VERSION {
            Some(version) => RouterVersion::Exact(Version::parse(version)?),
            None => RouterVersion::Latest,
        };

        let api_key_override = match std::env::var(RoverEnvKey::Key.to_string()) {
            Ok(key) => Some(key),
            Err(_err) => None,
        };
        let home_override = match std::env::var(RoverEnvKey::Home.to_string()) {
            Ok(home) => Some(home),
            Err(_err) => None,
        };

        let credential = Profile::get_credential(
            &profile.profile_name,
            &Config::new(home_override.as_ref(), api_key_override)?,
        )?;

        // Set up an updater config, but only if we're not overriding the version ourselves. If
        // we are then we don't need one, so it becomes None.
        let federation_updater_config = match self.opts.supergraph_opts.federation_version {
            Some(_) => None,
            None => Some(FederationUpdaterConfig {
                studio_client_config: client_config.clone(),
                elv2_licence_accepter: elv2_license_accepter,
                skip_update,
            }),
        };

        let composition_runner = composition_pipeline
            .runner(
                exec_command_impl,
                write_file_impl.clone(),
                client_config.service()?,
                fetch_remote_subgraph_factory.boxed_clone(),
                self.opts.subgraph_opts.subgraph_polling_interval,
                tmp_config_dir_path.clone(),
                true,
                federation_updater_config,
            )
            .await?;

        let mut composition_messages = composition_runner.run();

        // Sit in a loop and wait for the composition to actually succeed, once it does then
        // we can progress
        let supergraph_schema;
        loop {
            match composition_messages.next().await {
                Some(CompositionEvent::Started) => {
                    if let Ok(ref binary) = composition_pipeline.state.supergraph_binary {
                        eprintln!(
                            "composing supergraph with Federation {}",
                            binary.version()
                        );
                    }
                },
                Some(CompositionEvent::Success(success)) => {
                    supergraph_schema = success.supergraph_sdl;
                    break;
                },
                Some(CompositionEvent::Error(CompositionError::Build {source,..})) => {
                    let number_of_subgraphs = source.len();
                    let error_to_output = RoverError::from(RoverClientError::BuildErrors{ source, num_subgraphs: number_of_subgraphs });
                    eprintln!("{}", error_to_output)
                }
                Some(CompositionEvent::Error(err)) => {
                    errln!("Error occurred when composing supergraph\n{}", err)
                }
                Some(_) => {},
                None => return Err(RoverError::new(anyhow!("Composition Events Stream closed before supergraph schema could successfully compose")))
            }
        }

        // This RouterAddress hasn't been fully processed. It only represents the CLI option or
        // default, but we still have to reckon with the config-set address (if one exists). See
        // the reassignment of the variable below for details
        let router_address = RouterAddress::new(
            self.opts
                .supergraph_opts
                .supergraph_address
                .map(RouterHost::CliOption),
            self.opts
                .supergraph_opts
                .supergraph_port
                .map(RouterPort::CliOption),
        );

        let run_router = RunRouter::default()
            .install(
                router_version,
                client_config.clone(),
                override_install_path,
                elv2_license_accepter,
                skip_update,
            )
            .await?
            .load_config(&read_file_impl, router_address, router_config_path)
            .await?
            .load_remote_config(
                who_am_i_service,
                graph_ref.clone(),
                Some(credential.clone()),
            )
            .await;
        // This RouterAddress has some logic figuring out _which_ of the potentially multiple
        // address options we should use (eg, CLI, config, env var, or default). It will be used in
        // the overrides for the temporary config we set for hot-reloading the router, but also as
        // a message to the user for where to find their router
        let router_address = *run_router.state.config.address();
        let hot_reload_overrides = HotReloadConfigOverrides::builder()
            .address(router_address)
            .build();

        infoln!(
            "Attempting to start router at {}.",
            router_address.pretty_string()
        );

        let mut run_router = run_router
            .run(
                FsWriteFile::default(),
                TokioSpawn::default(),
                &tmp_config_dir_path,
                client_config.clone(),
                &supergraph_schema,
                credential,
            )
            .await?
            .watch_for_changes(write_file_impl, composition_messages, hot_reload_overrides)
            .await;

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    eprintln!("\nreceived shutdown signal, stopping `rover dev` processes...");
                    run_router.shutdown();
                    break
                },
                Some(router_log) = run_router.router_logs().next() => {
                    match router_log {
                        Ok(router_log) => {
                            if !router_log.to_string().is_empty() {
                                eprintln!("{}", router_log);
                            }
                        }
                        Err(RunRouterBinaryError::BinaryExited(res)) => {
                            match res {
                                Ok(status) => {
                                    match status.code() {
                                        None => {
                                            eprintln!("Router process terminal by signal");
                                        }
                                        Some(code) => {
                                            eprintln!("Router process exited with status code: {code}");
                                        }
                                    }

                                }
                                Err(err) => {
                                    tracing::error!("Router process exited without status code. Error: {err}")
                                }
                            }
                            eprintln!("\nRouter binary exited, stopping `rover dev` processes...");
                            break;
                        }
                        Err(err) => {
                            tracing::error!("{:?}", err);
                        }
                    }
                },
                else => break,
            }
        }
        Ok(RoverOutput::EmptySuccess)
    }
}
