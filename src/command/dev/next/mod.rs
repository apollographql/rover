#![warn(missing_docs)]

use anyhow::anyhow;
use apollo_federation_types::config::RouterVersion;
use camino::Utf8PathBuf;
use futures::StreamExt;
use houston::{Config, Profile};
use router::{
    install::InstallRouter,
    run::RunRouter,
    watchers::{file::FileWatcher, router_config::RouterConfigWatcher},
};
use rover_client::operations::config::who_am_i::WhoAmI;
use tower::{Service, ServiceExt};

use crate::{
    command::Dev,
    composition::runner::OneShotComposition,
    subtask::{Subtask, SubtaskRunUnit},
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::{TokioCommand, TokioSpawn},
            read_file::FsReadFile,
            write_file::{FsWriteFile, WriteFileRequest},
        },
    },
    RoverError, RoverOutput, RoverResult,
};

use self::router::{
    binary::RouterLog,
    config::{RouterAddress, RunRouterConfig},
};

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
        let mut write_file_impl = FsWriteFile::default();
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

        let supergraph_yaml = self.opts.supergraph_opts.clone().supergraph_config_path;
        let federation_version = self.opts.supergraph_opts.federation_version.clone();
        let profile = self.opts.plugin_opts.profile.clone();
        let graph_ref = self.opts.supergraph_opts.graph_ref.clone();

        let one_shot_composition = OneShotComposition::builder()
            .client_config(client_config.clone())
            .profile(profile.clone())
            .elv2_license_accepter(elv2_license_accepter)
            .skip_update(skip_update)
            .and_federation_version(federation_version)
            .and_graph_ref(graph_ref.clone())
            .and_supergraph_yaml(supergraph_yaml)
            .and_override_install_path(override_install_path.clone())
            .build();

        let supergraph_schema = one_shot_composition
            .compose(&read_file_impl, &write_file_impl, &exec_command_impl)
            .await?
            .supergraph_sdl;

        // TODO: figure out how to actually get this; maybe based on fed version? didn't see a cli
        // opt
        let router_version = RouterVersion::Latest;

        let credential =
            Profile::get_credential(&profile.profile_name, &Config::new(None::<&String>, None)?)?;

        let service = client_config
            .get_authenticated_client(&profile)?
            .service()?;
        let service = WhoAmI::new(service);

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
            .load_remote_config(service, graph_ref, Some(credential))
            .await
            .run(
                FsWriteFile::default(),
                TokioSpawn::default(),
                &tmp_config_dir_path,
                client_config,
                supergraph_schema,
            )
            .await?
            .watch_for_changes(write_file_impl)
            .await;

        while let Some(router_log) = run_router.router_logs().next().await {
            match router_log {
                Ok(RouterLog::Stdout(router_log)) => {
                    tracing::info!("{}", router_log);
                }
                Ok(RouterLog::Stderr(router_log)) => {
                    tracing::error!("{:?}", router_log);
                }
                Err(err) => {
                    tracing::error!("{:?}", err);
                }
            }
        }

        Ok(RoverOutput::EmptySuccess)
    }
}
