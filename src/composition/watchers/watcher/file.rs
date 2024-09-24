use std::sync::{Arc, OnceLock};

use camino::Utf8PathBuf;
use rover_std::{errln, Fs, RoverStdError};
use tap::TapFallible;
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;

use crate::composition::watchers::subtask::SubtaskHandleUnit;

#[derive(Clone, Debug)]
pub struct FileWatcher {
    path: Utf8PathBuf,
}

impl FileWatcher {
    pub fn new(path: Utf8PathBuf) -> Self {
        Self { path }
    }

    fn read_file(&self) -> Result<String, RoverStdError> {
        Fs::read_file(&self.path).tap_err(|err| {
            tracing::error!("Could not read file: {:?}", err);
            errln!("error reading file: {:?}", err);
        })
    }
}

impl SubtaskHandleUnit for FileWatcher {
    type Output = String;

    fn handle(self, sender: tokio::sync::mpsc::UnboundedSender<Self::Output>) -> CancellationToken {
        let cancellation_token = CancellationToken::new();
        match self.read_file() {
            Ok(contents) => {
                let _ = sender.send(contents).tap_err(|err| {
                    tracing::error!(
                        "Could not push initial file watch message. Error: {:?}",
                        err
                    );
                });
            }
            Err(err) => {
                tracing::error!(
                    "Could not push initial file watch message. Error: {:?}",
                    err
                );
            }
        }
        tokio::task::spawn({
            let cancellation_token = cancellation_token.clone();
            let abort_handle = Arc::new(OnceLock::new());
            async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        let abort_handle = abort_handle.clone();
                        if let Some(abort_handle) = abort_handle.get() {
                            abort_handle.abort();
                        }
                    }
                    _ = {
                        let abort_handle = abort_handle.clone();
                        async move {
                            tracing::debug!("!!!!!");
                            let (file_tx, mut file_rx) = unbounded_channel();
                            let join_handle = Fs::watch_file(self.path.clone(), file_tx);
                            let _ = abort_handle.set(join_handle).tap_err(|err| tracing::error!("{:?}", err));
                            while file_rx.recv().await.is_some() {
                                match Fs::read_file(self.path.clone()) {
                                    Ok(contents) => {
                                        let _ = sender.send(contents).tap_err(|err| {
                                            tracing::error!(
                                                "Could not send new file contents for file ({}). Error: {:?}",
                                                self.path,
                                                err
                                            )
                                        });
                                    }
                                    Err(err) => {
                                        tracing::error!("Could not read file: {:?}", err);
                                        errln!("error reading file: {:?}", err);
                                    }
                                }
                            }
                        }
                    } => {}
                }
            }
        });
        cancellation_token
    }
}
