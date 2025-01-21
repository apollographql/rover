use std::sync::OnceLock;

use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::{stream::BoxStream, StreamExt, TryFutureExt};
use rover_std::{errln, Fs, RoverStdError};
use tap::TapFallible;
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::{CancellationToken, DropGuard};
use tower::{Service, ServiceExt};

use crate::composition::supergraph::config::{
    error::ResolveSubgraphError,
    full::{FullyResolveSubgraphService, FullyResolvedSubgraph},
};

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

    pub async fn fetch(&self) -> Result<String, RoverStdError> {
        Fs::read_file(self.path.clone())
    }

    /// Watch a file
    ///
    /// When a file is removed, the internal rover-std::fs filewatcher will be cancelled. The
    /// composition filewatcher's stream will still be active, however
    ///
    /// Development note: in the future, we might consider a way to kill the watcher when the
    /// rover-std::fs filewatcher dies. Right now, the stream remains active and we can
    /// indefinitely loop on a close filewatcher
    pub async fn watch(&self) -> BoxStream<'static, String> {
        let (file_tx, file_rx) = unbounded_channel();
        let output = UnboundedReceiverStream::new(file_rx);
        let cancellation_token = Fs::watch_file(self.path.as_path().into(), file_tx, None);
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

                        match result {
                            Ok(_) => Fs::read_file(path)
                                .tap_err(|err| tracing::error!("Could not read file: {:?}", err))
                                .ok(),
                            Err(err) => {
                                errln!("error reading file: {:?}", err);
                                None
                            }
                        }
                    }
                }
            })
            .boxed()
    }
}

/// File watcher specifically for files related to composition
#[derive(Debug, Clone, Getters)]
pub struct SubgraphFileWatcher {
    /// The filepath to watch
    path: Utf8PathBuf,
    resolver: FullyResolveSubgraphService,
    drop_guard: Arc<Mutex<Option<DropGuard>>>,
}

impl SubgraphFileWatcher {
    /// Create a new filewatcher
    pub fn new(path: Utf8PathBuf, resolver: FullyResolveSubgraphService) -> Self {
        Self {
            path,
            resolver,
            drop_guard: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn fetch(mut self) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        self.resolver.ready().await?.call(()).await
    }

    /// Watch a file
    ///
    /// When a file is removed, the internal rover-std::fs filewatcher will be cancelled. The
    /// composition filewatcher's stream will still be active, however
    ///
    /// Development note: in the future, we might consider a way to kill the watcher when the
    /// rover-std::fs filewatcher dies. Right now, the stream remains active and we can
    /// indefinitely loop on a close filewatcher
    pub async fn watch(
        self,
        cancellation_token: CancellationToken,
    ) -> BoxStream<'static, FullyResolvedSubgraph> {
        let (file_tx, file_rx) = unbounded_channel();
        let output = UnboundedReceiverStream::new(file_rx);
        let cancellation_token = Fs::watch_file(
            self.path.as_path().into(),
            file_tx,
            Some(cancellation_token),
        );
        {
            let mut drop_guard = self.drop_guard.lock().await;
            let _ = drop_guard.insert(cancellation_token.clone().drop_guard());
        }

        output
            .filter_map({
                let resolver = self.resolver.clone();
                move |result| {
                    let cancellation_token = cancellation_token.clone();
                    let mut resolver = resolver.clone();
                    async move {
                        // We cancel the filewatching when the file has been removed because it
                        // can no longer be watched
                        if let Err(RoverStdError::FileRemoved { file }) = &result {
                            tracing::error!("Closing file watcher for {file}");
                            errln!("Closing file watcher for {file:?}");
                            cancellation_token.cancel();
                        }

                        match result {
                            Ok(_) => resolver
                                .ready()
                                .and_then(|service| service.call(()))
                                .await
                                .tap_err(|err| tracing::error!("Could not read file: {:?}", err))
                                .ok(),
                            Err(err) => {
                                errln!("error reading file: {:?}", err);
                                None
                            }
                        }
                    }
                }
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Write, time::Duration};

    use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
    use speculoos::prelude::*;
    use tokio::time::timeout;
    use tower::ServiceExt;
    use tracing_test::traced_test;

    use super::*;
    use crate::composition::supergraph::config::full::file::ResolveFileSubgraph;
    use crate::composition::supergraph::config::unresolved::UnresolvedSubgraph;

    #[tokio::test]
    #[traced_test(level = "error")]
    async fn it_watches() {
        let root = tempfile::Builder::new().tempdir().unwrap();
        //let root = TempDir::new().unwrap();
        let supergraph_config_root = Utf8PathBuf::from_path_buf(root.path().to_path_buf()).unwrap();
        let path = supergraph_config_root.join("supergraph.yaml");
        let subgraph_name = "file-subgraph";
        let routing_url = "https://example.com/graphql";
        let resolve_file_subgraph = ResolveFileSubgraph::builder()
            .supergraph_config_root(supergraph_config_root)
            .path(path.clone())
            .unresolved_subgraph(UnresolvedSubgraph::new(
                subgraph_name.to_string(),
                SubgraphConfig {
                    schema: SchemaSource::File { file: path.clone() },
                    routing_url: Some(routing_url.to_string()),
                },
            ))
            .build()
            .boxed_clone();

        let mut writeable_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(path.clone())
            .expect("Cannot open file");

        let sdl = "type Query { test: String }";
        writeable_file
            .write_all(sdl.as_bytes())
            .expect("couldn't write to file");

        let watcher = SubgraphFileWatcher::new(path.clone(), resolve_file_subgraph);
        let mut watching = watcher.watch(CancellationToken::default()).await;
        let _ = tokio::time::sleep(Duration::from_millis(500)).await;

        let mut writeable_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path.clone())
            .expect("Cannot open file");

        let sdl = "type Query { test: String! }";

        writeable_file
            .write_all(sdl.as_bytes())
            .expect("couldn't write to file");

        let output = timeout(Duration::from_secs(5), watching.next()).await;

        let expected = FullyResolvedSubgraph::builder()
            .name(subgraph_name.to_string())
            .routing_url(routing_url.to_string())
            .schema(sdl.to_string())
            .schema_source(SchemaSource::File { file: path })
            .build();
        assert_that!(&output)
            .is_ok()
            .is_some()
            .is_equal_to(expected);
    }
}
