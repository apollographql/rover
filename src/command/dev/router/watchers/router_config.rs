use futures::StreamExt;
use tap::TapFallible;
use tokio_util::sync::CancellationToken;

use crate::{
    command::dev::router::{
        config::RouterConfig, hot_reload::RouterUpdateEvent, watchers::file::FileWatcher,
    },
    subtask::SubtaskHandleUnit,
};

/// Watches for router config changes
pub struct RouterConfigWatcher {
    file_watcher: FileWatcher,
}

impl RouterConfigWatcher {
    pub const fn new(file_watcher: FileWatcher) -> Self {
        Self { file_watcher }
    }
}

impl SubtaskHandleUnit for RouterConfigWatcher {
    type Output = RouterUpdateEvent;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
        cancellation_token: Option<CancellationToken>,
    ) {
        let cancellation_token = cancellation_token.unwrap_or_default();
        tokio::spawn(async move {
            cancellation_token
                .run_until_cancelled(async move {
                    while let Some(router_config) = self.file_watcher.clone().watch().next().await {
                        let _ = sender
                            .send(RouterUpdateEvent::ConfigChanged {
                                config: RouterConfig::new(router_config),
                            })
                            .tap_err(|err| tracing::error!("{:?}", err));
                    }
                })
                .await;
        });
    }
}
