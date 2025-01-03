use std::{collections::HashMap, marker::Send, pin::Pin, time::Duration};

use futures::{Stream, StreamExt};
use rover_client::operations::subgraph::introspect::{
    SubgraphIntrospectError, SubgraphIntrospectResponse,
};
use tower::{util::BoxCloneService, Service, ServiceBuilder, ServiceExt};

use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError,
        full::{FullyResolveSubgraph, FullyResolvedSubgraph},
    },
    subtask::{Subtask, SubtaskRunUnit},
    watch::Watch,
};

/// Subgraph introspection
#[derive(Debug, Clone)]
pub struct SubgraphIntrospection {
    resolver: FullyResolveSubgraph,
    polling_interval: Duration,
}

//TODO: impl retry (needed at least for dev)
impl SubgraphIntrospection {
    pub fn new(resolver: FullyResolveSubgraph, polling_interval: Duration) -> Self {
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
                    Ok(subgraph) => Some(subgraph),
                    Err(err) => {
                        tracing::error!("{:?}", err);
                        None
                    }
                }
            })
            .boxed()
    }
}
