use crate::{
    command::dev::{event::Event, introspect::UnknownIntrospectRunner, protocol::SubgraphKey},
    RoverError, RoverErrorSuggestion, RoverResult,
};

use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use camino::{Utf8Path, Utf8PathBuf};
use futures::prelude::*;
use reqwest::blocking::Client;
use rover_client::{
    blocking::StudioClient,
    operations::subgraph::fetch::{self, SubgraphFetchInput},
    shared::GraphRef,
};
use rover_std::{Emoji, Fs};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::instrument::WithSubscriber;
use url::Url;

#[derive(Debug)]
pub struct SubgraphSchemaSource {
    schema_watcher_kind: SubgraphSchemaWatcherKind,
    subgraph_key: SubgraphKey,
}

impl SubgraphSchemaSource {
    pub fn new_from_file_path<P>(subgraph_key: SubgraphKey, path: P) -> RoverResult<Self>
    where
        P: AsRef<Utf8Path>,
    {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::File(path.as_ref().to_path_buf()),
            subgraph_key,
        })
    }

    pub fn new_from_url(
        subgraph_key: SubgraphKey,
        client: Client,
        polling_interval: u64,
        headers: Option<HashMap<String, String>>,
    ) -> RoverResult<Self> {
        let (_, url) = subgraph_key.clone();
        let headers = headers.map(|header_map| header_map.into_iter().collect());
        let introspect_runner = UnknownIntrospectRunner::new(url, client, headers);
        Self::new_from_introspect_runner(subgraph_key, introspect_runner, polling_interval)
    }

    pub fn new_from_sdl(subgraph_key: SubgraphKey, sdl: String) -> RoverResult<Self> {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::Once(sdl),
            subgraph_key,
        })
    }

    pub fn new_from_graph_ref(
        graph_ref: &str,
        graphos_subgraph_name: String,
        routing_url: Option<Url>,
        yaml_subgraph_name: String,
        client: &StudioClient,
    ) -> RoverResult<Self> {
        // given a graph_ref and subgraph, run subgraph fetch to
        // obtain SDL and add it to subgraph_definition.
        let response = fetch::run(
            SubgraphFetchInput {
                graph_ref: GraphRef::from_str(graph_ref)?,
                subgraph_name: graphos_subgraph_name.clone(),
            },
            client,
        )
        .map_err(RoverError::from)?;
        let routing_url = match (routing_url, response.sdl.r#type) {
            (Some(routing_url), _) => routing_url,
            (
                None,
                rover_client::shared::SdlType::Subgraph {
                    routing_url: Some(graph_registry_routing_url),
                },
            ) => graph_registry_routing_url.parse().context(format!(
                "Could not parse graph registry routing url {}",
                graph_registry_routing_url
            ))?,
            (None, _) => {
                return Err(RoverError::new(anyhow!(
                    "Could not find routing URL in GraphOS for subgraph {graphos_subgraph_name}"
                ))
                .with_suggestion(RoverErrorSuggestion::AddRoutingUrlToSupergraphYaml)
                .with_suggestion(
                    RoverErrorSuggestion::PublishSubgraphWithRoutingUrl {
                        subgraph_name: yaml_subgraph_name,
                        graph_ref: graph_ref.to_string(),
                    },
                ));
            }
        };
        Self::new_from_sdl((yaml_subgraph_name, routing_url), response.sdl.contents)
    }

    pub fn new_from_introspect_runner(
        subgraph_key: SubgraphKey,
        unknown_introspect_runner: UnknownIntrospectRunner,
        polling_interval: u64,
    ) -> RoverResult<Self> {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::Introspect(
                unknown_introspect_runner,
                polling_interval,
            ),
            subgraph_key,
        })
    }

    /// Watch a subgraph source for changes, returning a stream
    pub fn into_stream(self) -> impl Stream<Item = Event> {
        let subgraph_key = self.subgraph_key.clone();
        match self.schema_watcher_kind {
            SubgraphSchemaWatcherKind::Introspect(unknown_introspect_runner, polling_interval_seconds) => {
                let (watch_sender, watch_receiver) = mpsc::channel(1);
                let endpoint = unknown_introspect_runner.endpoint();
                eprintln!(
                    "{}polling {} every {} {}",
                    Emoji::Listen,
                    &endpoint,
                    polling_interval_seconds,
                    match polling_interval_seconds {
                        1 => "second",
                        _ => "seconds",
                    }
                );
                match unknown_introspect_runner.run_and_get_introspect_runner() {
                    Ok((schema, introspect_runner)) => {
                        let task = async move {
                            loop {
                                match introspect_runner.run() {
                                    Ok(schema) => {
                                        if watch_sender.send(schema).await.is_err() {
                                            tracing::debug!("failed to push schema to stream. this is likely because `rover dev` is shutting down");
                                            break;
                                        }
                                        tokio::time::sleep(std::time::Duration::from_secs(
                                            polling_interval_seconds,
                                        ))
                                        .await;
                                    }
                                    Err(e) => {
                                        eprintln!("{e}");
                                    }
                                }
                            }
                        };
                        tokio::task::spawn(task.with_current_subscriber());
                        stream::once(future::ready(schema)).chain(ReceiverStream::new(watch_receiver)).boxed()
                    },
                    Err(e) => {
                        eprintln!("{e}");
                        stream::empty().boxed()
                    }
                }
            }
            SubgraphSchemaWatcherKind::File(path) => {
                if !path.exists() {
                    eprintln!("Subgraph schema at path '{path}' does not exist.");
                    stream::empty().boxed()
                } else {
                    match Fs::read_file(&path) {
                        Ok(_) => Fs::watch_file(path.clone())
                            .filter_map(move |_| {
                                let path = path.clone();
                                async move {
                                    let result = Fs::read_file(&path);
                                    if let Err(e) = &result {
                                        eprintln!("{e}");
                                    }
                                    result.ok()
                                }
                            })
                            .boxed(),
                        Err(err) => {
                            eprintln!("{err}");
                            stream::empty().boxed()
                        }
                    }
                }
            }
            SubgraphSchemaWatcherKind::Once(schema) => {
                stream::once(future::ready(schema.to_string()))
                .boxed()
            }
        }.map(move |schema| {
            Event::UpdateSubgraphSchema { subgraph_key: self.subgraph_key.clone(), schema }
        }).chain(stream::iter(vec![Event::RemoveSubgraphSchema { subgraph_key }]))
    }

    pub fn set_schema_refresher(&mut self, new_refresher: SubgraphSchemaWatcherKind) {
        self.schema_watcher_kind = new_refresher;
    }

    pub fn get_name(&self) -> String {
        self.subgraph_key.0.to_string()
    }
}

#[derive(Debug, Clone)]
pub enum SubgraphSchemaWatcherKind {
    /// Poll an endpoint via introspection
    Introspect(UnknownIntrospectRunner, u64),
    /// Watch a file on disk
    File(Utf8PathBuf),
    /// Don't ever update, schema is only pulled once
    Once(String),
}
