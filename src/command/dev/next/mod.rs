#![warn(missing_docs)]

use std::time::Duration;

use anyhow::anyhow;
use apollo_federation_types::config::RouterVersion;
use camino::Utf8PathBuf;
use chrono::Duration;
use futures::task::Spawn;
use houston::{Config, Profile};
use http::StatusCode;
use reqwest::Request;
use router::{
    install::InstallRouter,
    run::RunRouter,
    watchers::{file::FileWatcher, router_config::RouterConfigWatcher},
};
use rover_client::operations::config::who_am_i::{self, WhoAmI};
use tokio::time::sleep;
use tower::ServiceBuilder;

use crate::{
    command::Dev,
    composition::runner::OneShotComposition,
    subtask::{Subtask, SubtaskRunUnit},
    utils::{
        client::StudioClientConfig,
        effect::{exec::TokioSpawn, read_file::FsReadFile, write_file::FsWriteFile},
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
        let router_address = RouterAddress::new(
            self.opts.supergraph_opts.supergraph_address,
            self.opts.supergraph_opts.supergraph_port,
        );

        let tmp_dir = tempfile::Builder::new().prefix("supergraph").tempdir()?;
        let tmp_config_dir_path = Utf8PathBuf::try_from(tmp_dir.into_path())?;

        let router_config_path = match self.opts.supergraph_opts.router_config_path.as_ref() {
            Some(path) => path.to_owned(),
            None => {
                let tmp_router_config_path = tmp_config_dir_path.join("router.yaml");
                tmp_router_config_path
            }
        };

        let _config = RunRouterConfig::default()
            .with_address(router_address)
            .with_config(&read_file_impl, &router_config_path)
            .await
            .map_err(|err| RoverError::new(anyhow!("{}", err)))?;

        let file_watcher = FileWatcher::new(router_config_path.clone());
        let router_config_watcher = RouterConfigWatcher::new(file_watcher);

        let (_events, subtask) = Subtask::new(router_config_watcher);
        let _abort_handle = subtask.run();

        let supergraph_yaml = self.opts.supergraph_opts.clone().supergraph_config_path;
        let federation_version = self.opts.supergraph_opts.federation_version.clone();
        let profile = self.opts.plugin_opts.profile.clone();
        let graph_ref = self.opts.supergraph_opts.graph_ref.clone();
        let composition_output = tmp_config_dir_path.join("supergraph.graphql");

        let one_off_composition = OneShotComposition::builder()
            .client_config(client_config.clone())
            .profile(profile.clone())
            .elv2_license_accepter(elv2_license_accepter)
            .skip_update(skip_update)
            .output_file(composition_output)
            .and_federation_version(federation_version)
            .and_graph_ref(graph_ref)
            .and_supergraph_yaml(supergraph_yaml)
            .and_override_install_path(override_install_path.clone())
            .build();

        // FIXME: send this off to the router binary
        let _composition_output = one_off_composition.compose().await?;

        let run_router = RunRouter::default();
        // TODO: figure out how to actually get this; maybe based on fed version? didn't see a cli
        // opt
        let router_version = RouterVersion::Latest;
        let run_router = run_router
            .install::<InstallRouter>(
                router_version,
                client_config.clone(),
                override_install_path,
                elv2_license_accepter,
                skip_update,
            )
            .await?;

        let service = client_config
            .get_authenticated_client(&profile)?
            .service()?;

        let run_router = run_router
            .load_config(&read_file_impl, router_address, router_config_path)
            .await?;

        // TODO: better; weird to call config, weird to get credential this way
        // FIXME: unwraps
        let credential =
        // FIXME: error over first None, the override home arg
        // 1. type annotations needed
        //    multiple `impl`s satisfying `_: AsRef<Utf8Path>` found in the `camino` crate:
        //    - impl AsRef<Utf8Path> for Utf8Path;
        //    - impl AsRef<Utf8Path> for camino::Utf8PathBuf;
        //    - impl AsRef<Utf8Path> for std::string::String;
        //    - impl AsRef<Utf8Path> for str; [E0283]
        //  2. required by a bound introduced by this call [E0283]
        //  3. consider specifying the generic argument: `::<&_>` [E0283]
            Profile::get_credential(&profile.profile_name, &Config::new(None, None).unwrap())
                .unwrap();

        let mut service = WhoAmI::new(service);

        let run_router = run_router
            // TODO: figure out if I can just pass None instead for the credential and let the
            // internal workings of it sort it out
            .load_remote_config(service, graph_ref, Some(credential))
            .await;

        // TODO: figure out if all the right files will be in the tmp_config_dir_path;
        // watch_for_changes() wants a config.yaml in there, but I'm not sure it exists yet unless
        // written out by some internal logic
        run_router.watch_for_changes(write_file_impl, TokioSpawn::default(), &tmp_config_dir_path);

        // TODO: this should actually come from the config if we have it; eg, the difference
        // between /health, the default, and a custom /healthz
        // see: https://www.apollographql.com/docs/graphos/routing/self-hosted/health-checks
        let mut healthcheck_endpoint = router_address.host().to_string();
        healthcheck_endpoint.push_str(":8088/health");

        // FIXME: unwrap
        let healthcheck_client = client_config.get_reqwest_client().unwrap();

        let healthcheck_request = healthcheck_client
            .get(healthcheck_endpoint)
            .build()
            .unwrap();

        // Wait for the router to become healthy before continuing by checking its health endpoint
        while !healthcheck_client
            .execute(healthcheck_request)
            .await
            .unwrap()
            .status()
            .is_success()
        {
            sleep(Duration::from_millis(100)).await;
        }

        // TODO: more stuff with dev, the router is alive

        Ok(RoverOutput::EmptySuccess)
    }
}
