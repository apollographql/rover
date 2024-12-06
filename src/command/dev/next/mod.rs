#![warn(missing_docs)]

use anyhow::anyhow;
use camino::Utf8PathBuf;
use futures::StreamExt;
use router::watchers::FileWatcher;
use tap::TapFallible;

use crate::{
    command::Dev,
    composition::runner::OneShotComposition,
    subtask::{Subtask, SubtaskHandleUnit, SubtaskRunUnit},
    utils::{client::StudioClientConfig, effect::read_file::FsReadFile},
    RoverError, RoverOutput, RoverResult,
};

use self::router::{
    config::{RouterAddress, RouterConfig, RunRouterConfig},
    hot_reload::RouterUpdateEvent,
};

mod router;

impl Dev {
    /// Runs rover dev
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        let read_file_impl = FsReadFile::default();
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

        let file_watcher = FileWatcher::new(router_config_path);
        let router_config_watcher = RouterConfigWatcher::new(file_watcher);

        let (_events, subtask) = Subtask::new(router_config_watcher);
        let _abort_handle = subtask.run();

        let supergraph_yaml = self.opts.supergraph_opts.clone().supergraph_config_path;
        let federation_version = self.opts.supergraph_opts.federation_version.clone();
        let profile = self.opts.plugin_opts.profile.clone();
        let graph_ref = self.opts.supergraph_opts.graph_ref.clone();
        let composition_output = tmp_config_dir_path.join("supergraph.graphql");

        let one_off_composition = OneShotComposition::builder()
            .client_config(client_config)
            .profile(profile)
            .elv2_license_accepter(self.opts.plugin_opts.elv2_license_accepter)
            .skip_update(self.opts.plugin_opts.skip_update)
            .output_file(composition_output)
            .and_federation_version(federation_version)
            .and_graph_ref(graph_ref)
            .and_supergraph_yaml(supergraph_yaml)
            .and_override_install_path(override_install_path)
            .build();

        // FIXME: send this off to the router binary
        let _composition_output = one_off_composition.compose().await?;

        Ok(RoverOutput::EmptySuccess)
    }
}

/// Watches for router config changes
struct RouterConfigWatcher {
    file_watcher: FileWatcher,
}

impl RouterConfigWatcher {
    fn new(file_watcher: FileWatcher) -> Self {
        Self { file_watcher }
    }
}

impl SubtaskHandleUnit for RouterConfigWatcher {
    type Output = RouterUpdateEvent;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
    ) -> tokio::task::AbortHandle {
        tokio::spawn(async move {
            while let Some(router_config) = self.file_watcher.clone().watch().next().await {
                let _ = sender
                    .send(RouterUpdateEvent::ConfigChanged {
                        config: RouterConfig::new(router_config),
                    })
                    .tap_err(|err| tracing::error!("{:?}", err));
            }
        })
        .abort_handle()
    }
}
