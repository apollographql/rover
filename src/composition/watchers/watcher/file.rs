use std::sync::OnceLock;

use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::{stream::BoxStream, StreamExt};
use rover_std::{errln, Fs, RoverStdError};
use tap::TapFallible;
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::DropGuard;

/// File watcher specifically for files related to composition
#[derive(Debug, Getters)]
pub struct FileWatcher {
    /// The filepath to watch
    path: Utf8PathBuf,
    drop_guard: OnceLock<DropGuard>,
}

impl FileWatcher {
    /// Create a new filewatcher
    pub fn new(path: Utf8PathBuf) -> Self {
        Self {
            path,
            drop_guard: OnceLock::new(),
        }
    }

    /// Watch a file
    ///
    /// When a file is removed, the internal rover-std::fs filewatcher will be cancelled. The
    /// composition filewatcher's stream will still be active, however
    ///
    /// Development note: in the future, we might consider a way to kill the watcher when the
    /// rover-std::fs filewatcher dies. Right now, the stream remains active and we can
    /// indefinitely loop on a close filewatcher
    pub fn watch(&self) -> BoxStream<'static, String> {
        let (file_tx, file_rx) = unbounded_channel();
        let output = UnboundedReceiverStream::new(file_rx);
        let cancellation_token = Fs::watch_file(self.path.as_path().into(), file_tx);
        self.drop_guard
            .set(cancellation_token.clone().drop_guard())
            .unwrap();

        output
            .filter_map({
                let path = self.path.clone();
                move |result| {
                    let cancellation_token = cancellation_token.clone();
                    let path = path.clone();
                    async move {
                        // We cancel the filewatching when the file has been removed because it
                        // can no longer be watched
                        if let Err(RoverStdError::FileRemoved { file }) = &result {
                            tracing::error!("Closing file watcher for {file}");
                            errln!("Closing file watcher for {file:?}");
                            cancellation_token.cancel();
                        }

                        result
                            .and_then(|_| {
                                Fs::read_file(path.clone()).tap_err(|err| {
                                    tracing::error!("Could not read file: {:?}", err);
                                    errln!("error reading file: {:?}", err);
                                })
                            })
                            .ok()
                    }
                }
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use speculoos::assert_that;
    use speculoos::option::OptionAssertions;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn it_watches() {
        let some_file = tempfile::Builder::new().tempfile().unwrap();
        let path = some_file.path().to_path_buf();
        let watcher = FileWatcher::new(Utf8PathBuf::from_path_buf(path.clone()).unwrap());
        let mut watching = watcher.watch();
        let _ = tokio::time::sleep(Duration::from_secs(2)).await;

        let mut writeable_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .expect("Cannot open file");

        writeable_file
            .write_all("some change".as_bytes())
            .expect("couldn't write to file");

        let mut output = None;
        while output.is_none() {
            let _ = tokio::time::sleep(Duration::from_secs(1)).await;
            output = watching.next().await;
        }

        assert_that(&output)
            .is_some()
            .matches(|actual| actual == "some change");

        let _ = some_file.close();
    }
}
