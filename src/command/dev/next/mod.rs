#![warn(missing_docs)]

use anyhow::anyhow;
use camino::Utf8PathBuf;
use futures::StreamExt;
use router::watchers::FileWatcher;
use tap::TapFallible;

use crate::{
    command::Dev,
    subtask::{Subtask, SubtaskHandleUnit, SubtaskRunUnit},
    utils::{client::StudioClientConfig, effect::read_file::FsReadFile},
    RoverError, RoverOutput, RoverResult,
};

use self::router::config::{RouterAddress, RunRouterConfig};

mod router;

impl Dev {
    /// Runs rover dev
    pub async fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        _client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        let read_file_impl = FsReadFile::default();
        let router_address = RouterAddress::new(
            self.opts.supergraph_opts.supergraph_address,
            self.opts.supergraph_opts.supergraph_port,
        );

        let router_config_path = match self.opts.supergraph_opts.router_config_path.as_ref() {
            Some(path) => path.to_owned(),
            None => {
                let tmp_dir = tempfile::Builder::new().prefix("supergraph").tempdir()?;
                let tmp_config_dir_path = Utf8PathBuf::try_from(tmp_dir.into_path())?;
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

// FIXME: use proper struct once we have it from the work for running the router binary
struct ReplaceMeWithProperRouterEventsStruct {
    #[allow(dead_code)]
    router_config: String,
}

impl SubtaskHandleUnit for RouterConfigWatcher {
    type Output = ReplaceMeWithProperRouterEventsStruct;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
    ) -> tokio::task::AbortHandle {
        tokio::spawn(async move {
            while let Some(router_config) = self.file_watcher.clone().watch().next().await {
                let _ = sender
                    .send(ReplaceMeWithProperRouterEventsStruct { router_config })
                    .tap_err(|err| tracing::error!("{:?}", err));
            }
        })
        .abort_handle()
    }
}
