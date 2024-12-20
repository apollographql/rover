use std::{collections::HashMap, marker::Send, pin::Pin, time::Duration};

use futures::{Stream, StreamExt};
use rover_client::operations::subgraph::introspect::{
    SubgraphIntrospectError, SubgraphIntrospectLayer, SubgraphIntrospectResponse,
};
use tower::{util::BoxCloneService, Service, ServiceBuilder, ServiceExt};

use crate::{
    composition::types::SubgraphUrl,
    subtask::{Subtask, SubtaskRunUnit},
    utils::client::StudioClientConfig,
    watch::Watch,
};

/// Subgraph introspection
#[derive(Debug, Clone)]
pub struct SubgraphIntrospection {
    endpoint: SubgraphUrl,
    client_config: StudioClientConfig,
    headers: Vec<(String, String)>,
    polling_interval: Duration,
}

//TODO: impl retry (needed at least for dev)
impl SubgraphIntrospection {
    pub fn new(
        endpoint: SubgraphUrl,
        headers: Option<Vec<(String, String)>>,
        client_config: StudioClientConfig,
        polling_interval: Duration,
    ) -> Self {
        Self {
            endpoint,
            client_config,
            headers: headers.unwrap_or_default(),
            polling_interval,
        }
    }

    pub async fn fetch(&self) -> Result<String, SubgraphIntrospectError> {
        let resp = self.service(true).ready().await?.call(()).await?;
        Ok(resp.result)
    }

    // TODO: better typing so that it's over some impl, not string; makes all watch() fns require
    // returning a string
    pub fn watch(&self) -> Pin<Box<dyn Stream<Item = String> + Send>> {
        let service = self.service(false);

        let watch = Watch::builder()
            .polling_interval(self.polling_interval.clone())
            .service(service)
            .build();
        let (watch_messages, watch_subtask) = Subtask::new(watch);
        watch_subtask.run();

        // Stream any subgraph changes, filtering out empty responses (None) while passing along
        // the sdl changes
        // this skips the first event, since the inner function always produces a result when it's
        // initialized
        watch_messages
            .skip(1)
            .filter_map(|change| async move {
                match change {
                    Ok(sdl) => Some(sdl.result),
                    Err(err) => {
                        tracing::error!("{:?}", err);
                        None
                    }
                }
            })
            .boxed()
    }

    fn service(
        &self,
        should_retry: bool,
    ) -> BoxCloneService<(), SubgraphIntrospectResponse, SubgraphIntrospectError> {
        let http_service = self.client_config.service().unwrap();
        let introspect_layer = SubgraphIntrospectLayer::new(
            self.endpoint.clone(),
            HashMap::from_iter(self.headers.clone().into_iter()),
            should_retry,
            self.client_config.retry_period(),
        )
        .unwrap();
        let service = ServiceBuilder::new()
            .layer(introspect_layer)
            .service(http_service);
        service.boxed_clone()
    }
}
