#![warn(missing_docs)]

use std::{io::stdin, str::FromStr};

use anyhow::anyhow;
use apollo_federation_types::config::{FederationVersion, RouterVersion};
use camino::Utf8PathBuf;
use futures::StreamExt;
use houston::{Config, Profile};
use router::{install::InstallRouter, run::RunRouter, watchers::file::FileWatcher};
use rover_client::operations::config::who_am_i::WhoAmI;
use rover_std::{errln, infoln, warnln};
use tower::ServiceExt;

use crate::{
    command::{dev::OVERRIDE_DEV_COMPOSITION_VERSION, dev::OVERRIDE_DEV_ROUTER_VERSION, Dev},
    composition::{
        pipeline::CompositionPipeline,
        supergraph::config::{
            full::introspect::MakeResolveIntrospectSubgraph,
            resolver::{
                fetch_remote_subgraph::MakeFetchRemoteSubgraph,
                fetch_remote_subgraphs::MakeFetchRemoteSubgraphs,
            },
        },
    },
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::{TokioCommand, TokioSpawn},
            read_file::FsReadFile,
            write_file::FsWriteFile,
        },
    },
    RoverError, RoverOutput, RoverResult,
};

use self::router::config::{RouterAddress, RunRouterConfig};

mod router;

impl Dev {
    /// Runs rover dev
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        let elv2_license_accepter = self.opts.plugin_opts.elv2_license_accepter;
        let skip_update = self.opts.plugin_opts.skip_update;
        let read_file_impl = FsReadFile::default();
        let write_file_impl = FsWriteFile::default();
        let exec_command_impl = TokioCommand::default();

        let router_address = RouterAddress::new(
            self.opts.supergraph_opts.supergraph_address,
            self.opts.supergraph_opts.supergraph_port,
        );

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
        let service = WhoAmI::new(service);

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
                    .and_then(|version| match FederationVersion::from_str(&version) {
                        Ok(version) => Some(version),
                        Err(err) => {
                            errln!("{err}");
                            tracing::error!("{:?}", err);
                            None
                        }
                    });

                version.clone()
            });

        let composition_pipeline = CompositionPipeline::default()
            .init(
                &mut stdin(),
                fetch_remote_subgraphs_factory,
                supergraph_config_path.clone(),
                graph_ref.clone(),
            )
            .await?
            .resolve_federation_version(
                resolve_introspect_subgraph_factory.clone(),
                fetch_remote_subgraph_factory.clone(),
                federation_version,
            )
            .await?
            .install_supergraph_binary(
                client_config.clone(),
                override_install_path.clone(),
                elv2_license_accepter,
                skip_update,
            )
            .await?;

        let composition_success = composition_pipeline
            .compose(&exec_command_impl, &read_file_impl, &write_file_impl, None)
            .await?;
        let supergraph_schema = composition_success.supergraph_sdl();

        let router_version = match &*OVERRIDE_DEV_ROUTER_VERSION {
            Some(version) => RouterVersion::from_str(version)?,
            None => RouterVersion::Latest,
        };

        let credential =
            Profile::get_credential(&profile.profile_name, &Config::new(None::<&String>, None)?)?;

        let composition_runner = composition_pipeline
            .runner(
                exec_command_impl,
                read_file_impl.clone(),
                write_file_impl.clone(),
                client_config.service()?,
                fetch_remote_subgraph_factory.boxed_clone(),
                self.opts.subgraph_opts.subgraph_polling_interval,
                tmp_config_dir_path.clone(),
            )
            .await?;

        let composition_messages = composition_runner.run();

        eprintln!(
            "composing supergraph with Federation {}",
            composition_pipeline.state.supergraph_binary.version()
        );

        let run_router = RunRouter::default()
            .install::<InstallRouter>(
                router_version,
                client_config.clone(),
                override_install_path,
                elv2_license_accepter,
                skip_update,
            )
            .await?
            .load_config(&read_file_impl, router_address, router_config_path)
            .await?
            .load_remote_config(service, graph_ref.clone(), Some(credential))
            .await;
        let router_address = *run_router.state.config.address();
        let mut run_router = run_router
            .run(
                FsWriteFile::default(),
                TokioSpawn::default(),
                &tmp_config_dir_path,
                client_config.clone(),
                supergraph_schema,
            )
            .await?
            .watch_for_changes(write_file_impl, composition_messages)
            .await;

        warnln!(
            "Do not run this command in production! It is intended for local development only."
        );

        infoln!("your supergraph is running! head to {router_address} to query your supergraph");

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
