use std::{marker::Send, pin::Pin, time::Duration};

use futures::{Stream, StreamExt};
use tower::{Service, ServiceExt};

use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError,
        full::{FullyResolveSubgraphService, FullyResolvedSubgraph},
    },
    subtask::{Subtask, SubtaskRunUnit},
    watch::Watch,
};

use rover_std::{errln, infoln};

/// Subgraph introspection
#[derive(Debug, Clone)]
pub struct SubgraphIntrospection {
    resolver: FullyResolveSubgraphService,
    polling_interval: Duration,
}

//TODO: impl retry (needed at least for dev)
impl SubgraphIntrospection {
    pub fn new(resolver: FullyResolveSubgraphService, polling_interval: Duration) -> Self {
        Self {
            resolver,
            polling_interval,
        }
    }

    pub async fn fetch(mut self) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        self.resolver.ready().await?.call(()).await
    }

    // TODO: better typing so that it's over some impl, not string; makes all watch() fns require
    // returning a string
    pub fn watch(self) -> Pin<Box<dyn Stream<Item = FullyResolvedSubgraph> + Send>> {
        let watch = Watch::builder()
            .polling_interval(self.polling_interval)
            .service(self.resolver.clone())
            .build();
        let (watch_messages, watch_subtask) = Subtask::new(watch);
        watch_subtask.run();

        // Stream any subgraph changes, filtering out empty responses (None) while passing along
        // the sdl changes
        // This skips the first event, since the inner function always produces a result when it's
        // initialized
        watch_messages
            .skip(1)
            .filter_map(|change| async move {
                match change {
                    Ok(subgraph) => {
                        infoln!(
                            "Connectivity restored for subgraph \"{}\".",
                            subgraph.name()
                        );
                        Some(subgraph)
                    }
                    Err(err) => {
                        errln!(
                            "{} \
Error communicating with subgraph.
* Schema changes will not be reflected.
* Inspect subgraph logs for more information.",
                            err
                        );
                        tracing::error!("{:?}", err);
                        None
                    }
                }
            })
            .boxed()
    }
}
