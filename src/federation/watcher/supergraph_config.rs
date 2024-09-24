//! Watch a `supergraph.yaml` file for changes

use apollo_federation_types::config::{ConfigError, SupergraphConfig};
use camino::Utf8PathBuf;
use rover_std::{Fs, RoverStdError};
use tokio::sync::mpsc::{channel, unbounded_channel, Receiver, Sender};

/// Spawns a task that watches the `supergraph.yaml` file at the given path for changes.
/// Returns a channel that delivers results.
pub(crate) async fn start_watching(path: Utf8PathBuf) -> Receiver<SupergraphFileEvent> {
    let (tx, rx) = channel(1);
    tokio::spawn(watch(path, tx));
    rx
}

#[derive(Debug)]
pub(crate) enum SupergraphFileEvent {
    SupergraphChanged(SupergraphConfig),
    SupergraphWasInvalid(ConfigError),
    FailedToReadSupergraph(RoverStdError),
}

/// Wait for the file to change and then read it, emitting [`SupergraphFileEvent`]s
///
/// This function returns when the channel closes.
async fn watch(path: Utf8PathBuf, tx: Sender<SupergraphFileEvent>) -> Option<()> {
    use SupergraphFileEvent::*;
    let (send_file_changed, mut file_changed) = unbounded_channel();
    Fs::watch_file(path.clone(), send_file_changed);
    while file_changed.recv().await.is_some() {
        let file_contents = match Fs::read_file(path.clone()) {
            Ok(val) => val,
            Err(err) => {
                tx.send(FailedToReadSupergraph(err)).await.ok()?;
                continue;
            }
        };
        match SupergraphConfig::new_from_yaml(&file_contents) {
            Ok(config) => {
                tx.send(SupergraphChanged(config)).await.ok()?;
            }
            Err(err) => {
                tx.send(SupergraphWasInvalid(err)).await.ok()?;
            }
        }
    }
    Some(())
}
