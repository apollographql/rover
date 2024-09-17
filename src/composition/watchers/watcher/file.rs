use camino::Utf8PathBuf;
use futures::{stream::BoxStream, StreamExt};
use tap::TapFallible;
use tokio::sync::mpsc::unbounded_channel;

use rover_std::Fs;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Clone)]
pub struct FileWatcher {
    pub path: Utf8PathBuf,
}

impl FileWatcher {
    pub fn new(path: Utf8PathBuf) -> FileWatcher {
        FileWatcher { path }
    }

    pub fn watch(self) -> BoxStream<'static, String> {
        let path = self.path;
        let (file_tx, file_rx) = unbounded_channel();
        let output = UnboundedReceiverStream::new(file_rx);
        Fs::watch_file(path.clone(), file_tx);
        output
            .filter_map(move |result| {
                let path = path.clone();
                async move {
                    result
                        .and_then(|_| {
                            Fs::read_file(path).tap_err(|err| {
                                tracing::error!(
                                    "Could not read router configuration file: {:?}",
                                    err
                                );
                                eprintln!("Could not read router configuration file.");
                            })
                        })
                        .ok()
                }
            })
            .boxed()
    }
}
