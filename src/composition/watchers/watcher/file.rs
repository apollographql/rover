use camino::Utf8PathBuf;
use futures::{stream::BoxStream, StreamExt};
use rover_std::{errln, Fs};
use tap::TapFallible;
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Clone, Debug)]
pub struct FileWatcher {
    path: Utf8PathBuf,
}

impl FileWatcher {
    pub fn new(path: Utf8PathBuf) -> Self {
        Self { path }
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
                                tracing::error!("Could not read file: {:?}", err);
                                errln!("error reading file: {:?}", err);
                            })
                        })
                        .ok()
                }
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::pin::Pin;
    use std::time::Duration;
    use std::{fs::OpenOptions, task::Context};

    use futures::future::join_all;
    use futures::{FutureExt, Sink};
    use tokio::time::sleep;

    use super::*;

    #[tokio::test]
    async fn it_watches() {
        let some_file = tempfile::Builder::new().tempfile().unwrap();
        let path = some_file.path().to_path_buf();
        let watcher = FileWatcher::new(Utf8PathBuf::from_path_buf(path.clone()).unwrap());
        let file_path = some_file.path();
        println!("AAA: file path we care about: {file_path:?}");
        let mut watching = watcher.watch();
        println!("AAA: after watching");

        // Internal to rover std fs is a blocking loop with a 1s debouncer; so, use 2s just in case
        let _ = tokio::time::sleep(Duration::from_secs(2)).await;

        let asdf = path.clone();
        let blah = tokio::spawn(async move {
            // Make a change to the file
            let mut writeable_file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(asdf)
                .expect("Cannot open file");

            writeable_file
                .write("some change".as_bytes())
                .expect("couldn't write to file");

            let next = watching.next().await.unwrap();
            next
        });

        //assert_eq!(next, "some change".to_string());

        let asdf = path.clone();
        let removal = tokio::spawn(async move {
            let _ = tokio::time::sleep(Duration::from_secs(2)).await;
            match std::fs::remove_file(asdf) {
                Ok(_) => println!("removed file from std::fs"),
                Err(err) => println!("failed to remove file from std::fs: {err:?}"),
            }
            "good".to_string()
        });

        let yah = join_all(vec![blah, removal]).await;
        println!("yah: {yah:?}");

        // Close the file to emit an event for rover-std fs to close the file watcher
        //match some_file.close() {
        //    Ok(_ok) => println!("closed just fine"),
        //    Err(err) => println!("error closing file: {err:?}"),
        //}

        let _ = sleep(Duration::from_secs(4)).await;

        println!("AAA: after await next");

        println!("AAA: after assert");

        println!("AAA: after trying to remove file");
    }
}
