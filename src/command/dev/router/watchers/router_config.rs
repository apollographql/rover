use futures::StreamExt;
use tap::TapFallible;

use crate::command::dev::FileWatcher;
use crate::{
    command::dev::router::{config::RouterConfig, hot_reload::RouterUpdateEvent},
    subtask::SubtaskHandleUnit,
};

/// Watches for router config changes
pub struct RouterConfigWatcher {
    file_watcher: FileWatcher,
}

impl RouterConfigWatcher {
    pub fn new(file_watcher: FileWatcher) -> Self {
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
