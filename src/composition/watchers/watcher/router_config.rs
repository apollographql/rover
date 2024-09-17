use futures::StreamExt;

use super::super::subtask::SubtaskHandleUnit;

use super::file::FileWatcher;

#[derive(Clone, Debug)]
pub enum RouterConfigMessage {
    Changed(String),
}

pub struct RouterConfigWatcher {
    file_watcher: FileWatcher,
}

impl RouterConfigWatcher {
    pub fn new(file_watcher: FileWatcher) -> RouterConfigWatcher {
        RouterConfigWatcher { file_watcher }
    }
}

impl SubtaskHandleUnit for RouterConfigWatcher {
    type Output = RouterConfigMessage;

    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
    ) -> tokio::task::AbortHandle {
        tokio::spawn(async move {
            while let Some(contents) = self.file_watcher.clone().watch().next().await {
                sender.send(RouterConfigMessage::Changed(contents));
            }
        })
        .abort_handle()
    }
}
