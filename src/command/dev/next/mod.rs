#![warn(missing_docs)]

use std::io::stdin;

use anyhow::anyhow;
use apollo_federation_types::config::RouterVersion;
use camino::Utf8PathBuf;
use futures::StreamExt;
use houston::{Config, Profile};
use router::{install::InstallRouter, run::RunRouter, watchers::file::FileWatcher};
use rover_client::operations::config::who_am_i::WhoAmI;

use crate::{
    command::Dev,
    composition::pipeline::CompositionPipeline,
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

        let _config = RunRouterConfig::default()
            .with_address(router_address)
            .with_config(&read_file_impl, router_config_path.as_ref())
            .await
            .map_err(|err| RoverError::new(anyhow!("{}", err)))?;

        let profile = &self.opts.plugin_opts.profile;
        let graph_ref = &self.opts.supergraph_opts.graph_ref;
        let supergraph_config_path = &self.opts.supergraph_opts.clone().supergraph_config_path;

        let service = client_config.get_authenticated_client(profile)?.service()?;
        let service = WhoAmI::new(service);

        let composition_pipeline = CompositionPipeline::default()
            .init(
                &mut stdin(),
                &client_config.get_authenticated_client(profile)?,
                supergraph_config_path.clone(),
                graph_ref.clone(),
            )
            .await?
            .resolve_federation_version(
                &client_config,
                &client_config.get_authenticated_client(profile)?,
                self.opts.supergraph_opts.federation_version.clone(),
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

        // TODO: figure out how to actually get this; maybe based on fed version? didn't see a cli
        // opt
        let router_version = RouterVersion::Latest;

        let credential =
            Profile::get_credential(&profile.profile_name, &Config::new(None::<&String>, None)?)?;

        let composition_runner = composition_pipeline
            .runner(
                exec_command_impl,
                read_file_impl.clone(),
                write_file_impl.clone(),
                profile,
                &client_config,
                self.opts.subgraph_opts.subgraph_polling_interval,
                tmp_config_dir_path.clone(),
            )
            .await?;

        let composition_messages = composition_runner.run();

        let mut run_router = RunRouter::default()
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
            .await
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

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    eprintln!("received shutdown signal, stopping `rover dev` processes...");
                    run_router.shutdown();
                    break
                },
                Some(router_log) = run_router.router_logs().next() => {
                    match router_log {
                        Ok(router_log) => {
                            eprintln!("{}", router_log);
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
