use std::{pin::Pin, sync::Arc};

use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::{stream::BoxStream, Future, StreamExt, TryFutureExt};
use rover_std::{errln, Fs, RoverStdError};
use tap::TapFallible;
use tokio::sync::{mpsc::unbounded_channel, Mutex};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::DropGuard;
use tower::{util::BoxCloneService, Service, ServiceExt};

use crate::composition::supergraph::config::{
    error::ResolveSubgraphError,
    full::{FullyResolveSubgraphService, FullyResolvedSubgraph},
};

pub type SubgraphFileWatcher = FileWatcher<FullyResolvedSubgraph, ResolveSubgraphError>;
pub type BasicFileWatcher = FileWatcher<String, RoverStdError>;

/// File watcher specifically for files related to composition
#[derive(Debug, Getters)]
pub struct FileWatcher<T, E> {
    /// The filepath to watch
    path: Utf8PathBuf,
    resolver: BoxCloneService<(), T, E>,
    drop_guard: Arc<Mutex<Option<DropGuard>>>,
}

// rust couldn't figure out how to derive this
impl<T, E> Clone for FileWatcher<T, E> {
    fn clone(&self) -> Self {
        FileWatcher {
            path: self.path.clone(),
            resolver: self.resolver.clone(),
            drop_guard: self.drop_guard.clone(),
        }
    }
}

impl FileWatcher<String, RoverStdError> {
    /// Create a new BasicFileWatcher
    pub fn basic(path: Utf8PathBuf) -> FileWatcher<String, RoverStdError> {
        let resolver = BasicFileResolver { path: path.clone() };
        let resolver = resolver.boxed_clone();
        Self {
            path,
            resolver,
            drop_guard: Arc::new(Mutex::new(None)),
        }
    }
}

impl FileWatcher<FullyResolvedSubgraph, ResolveSubgraphError> {
    /// Create a new filewatcher
    pub fn subgraph(path: Utf8PathBuf, resolver: FullyResolveSubgraphService) -> Self {
        Self {
            path,
            resolver,
            drop_guard: Arc::new(Mutex::new(None)),
        }
    }
}

impl<T, E> FileWatcher<T, E>
where
    T: 'static,
    E: std::error::Error + 'static,
{
    pub async fn fetch(mut self) -> Result<T, E> {
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
    pub async fn watch(self) -> BoxStream<'static, T> {
        let (file_tx, file_rx) = unbounded_channel();
        let output = UnboundedReceiverStream::new(file_rx);
        let cancellation_token = Fs::watch_file(self.path.as_path().into(), file_tx);
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
                            Err(RoverStdError::FileRemoved { file }) => {
                                tracing::error!("Closing file watcher for {file}");
                                errln!("Closing file watcher for {file:?}");
                                cancellation_token.cancel();
                                None
                            }
                            Err(err) => {
                                tracing::error!("File watcher error: {:?}", err);
                                cancellation_token.cancel();
                                None
                            }
                        }
                    }
                }
            })
            .boxed()
    }
}

#[derive(Clone, Debug)]
pub struct BasicFileResolver {
    path: Utf8PathBuf,
}

impl Service<()> for BasicFileResolver {
    type Response = String;
    type Error = RoverStdError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let path = self.path.clone();
        let fut = async move { Fs::read_file(path) };
        Box::pin(fut)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
    use assert_fs::{prelude::*, TempDir};
    use speculoos::prelude::*;
    use tokio::time::timeout;
    use tower::ServiceExt;
    use tracing_test::traced_test;

    use crate::composition::supergraph::config::full::file::ResolveFileSubgraph;
    use crate::composition::supergraph::config::unresolved::UnresolvedSubgraph;

    use super::*;

    #[tokio::test]
    #[traced_test(level = "error")]
    async fn it_watches() {
        let root = TempDir::new().unwrap();
        let supergraph_config_root = Utf8PathBuf::from_path_buf(root.path().to_path_buf()).unwrap();
        let child = root.child("supergraph.yaml");
        let path = Utf8PathBuf::from_path_buf(child.path().to_path_buf()).unwrap();
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

        let sdl = "type Query { test: String }";
        child.write_str(sdl).expect("couldn't write to file");

        let watcher = FileWatcher::subgraph(path.clone(), resolve_file_subgraph);
        // SubgraphFileWatcher has a DropGuard associated with it that cancels the underlying FileWatcher's CancellationToken when dropped, so we must retain a reference until this test finishes. This can be fixed if we migrate this to a `Subtask` implementation to make it safer and more explicit
        let _watcher = watcher.clone();
        let mut watching = watcher.watch().await;
        let _ = tokio::time::sleep(Duration::from_millis(500)).await;

        let sdl = "type Query { test: String! }";
        child.write_str(sdl).expect("couldn't write to file");

        let output = timeout(Duration::from_secs(5), watching.next()).await;

        let expected = FullyResolvedSubgraph::builder()
            .name(subgraph_name.to_string())
            .routing_url(routing_url.to_string())
            .schema(sdl.to_string())
            .build();
        assert_that!(&output)
            .is_ok()
            .is_some()
            .is_equal_to(expected);
    }
}
