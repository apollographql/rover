use crate::{
    command::dev::{
        introspect::{IntrospectRunnerKind, UnknownIntrospectRunner},
        protocol::{FollowerMessenger, SubgraphKey},
    },
    RoverError, RoverResult,
};
use anyhow::{anyhow, Context};
use std::collections::HashMap;
use std::str::FromStr;

use apollo_federation_types::build::SubgraphDefinition;
use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::unbounded;
use reqwest::blocking::Client;
use rover_client::blocking::StudioClient;
use rover_client::operations::subgraph::fetch;
use rover_client::operations::subgraph::fetch::SubgraphFetchInput;
use rover_client::shared::GraphRef;
use rover_std::{Emoji, Fs};
use url::Url;

#[derive(Debug)]
pub struct SubgraphSchemaWatcher {
    schema_watcher_kind: SubgraphSchemaWatcherKind,
    subgraph_key: SubgraphKey,
    message_sender: FollowerMessenger,
}

impl SubgraphSchemaWatcher {
    pub fn new_from_file_path<P>(
        subgraph_key: SubgraphKey,
        path: P,
        message_sender: FollowerMessenger,
    ) -> RoverResult<Self>
    where
        P: AsRef<Utf8Path>,
    {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::File(path.as_ref().to_path_buf()),
            subgraph_key,
            message_sender,
        })
    }

    pub fn new_from_url(
        subgraph_key: SubgraphKey,
        client: Client,
        message_sender: FollowerMessenger,
        polling_interval: u64,
        headers: Option<HashMap<String, String>>,
    ) -> RoverResult<Self> {
        let (_, url) = subgraph_key.clone();
        let headers = headers.map(|header_map| header_map.into_iter().collect());
        let introspect_runner =
            IntrospectRunnerKind::Unknown(UnknownIntrospectRunner::new(url, client, headers));
        Self::new_from_introspect_runner(
            subgraph_key,
            introspect_runner,
            message_sender,
            polling_interval,
        )
    }

    pub fn new_from_sdl(
        subgraph_key: SubgraphKey,
        sdl: String,
        message_sender: FollowerMessenger,
    ) -> RoverResult<Self> {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::Once(sdl),
            subgraph_key,
            message_sender,
        })
    }

    pub fn new_from_graph_ref(
        graph_ref: &str,
        routing_url: Option<Url>,
        subgraph_name: String,
        message_sender: FollowerMessenger,
        client: &StudioClient,
    ) -> RoverResult<Self> {
        // given a graph_ref and subgraph, run subgraph fetch to
        // obtain SDL and add it to subgraph_definition.
        let response = fetch::run(
            SubgraphFetchInput {
                graph_ref: GraphRef::from_str(graph_ref)?,
                subgraph_name: subgraph_name.clone(),
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
                return Err(anyhow!("Could not find routing URL in GraphOS for subgraph {subgraph_name}, try setting `routing_url`").into());
            }
        };
        Self::new_from_sdl(
            (subgraph_name, routing_url),
            response.sdl.contents,
            message_sender,
        )
    }

    pub fn new_from_introspect_runner(
        subgraph_key: SubgraphKey,
        introspect_runner: IntrospectRunnerKind,
        message_sender: FollowerMessenger,
        polling_interval: u64,
    ) -> RoverResult<Self> {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::Introspect(
                introspect_runner,
                polling_interval,
            ),
            subgraph_key,
            message_sender,
        })
    }

    pub fn get_subgraph_definition_and_maybe_new_runner(
        &self,
    ) -> RoverResult<(SubgraphDefinition, Option<SubgraphSchemaWatcherKind>)> {
        let (name, url) = self.subgraph_key.clone();
        let (sdl, refresher) = match &self.schema_watcher_kind {
            SubgraphSchemaWatcherKind::Introspect(introspect_runner_kind, polling_interval) => {
                match introspect_runner_kind {
                    IntrospectRunnerKind::Graph(graph_runner) => {
                        let sdl = graph_runner.run()?;
                        (sdl, None)
                    }
                    IntrospectRunnerKind::Subgraph(subgraph_runner) => {
                        let sdl = subgraph_runner.run()?;
                        (sdl, None)
                    }
                    IntrospectRunnerKind::Unknown(unknown_runner) => {
                        let (sdl, specific_runner) = unknown_runner.run()?;
                        (
                            sdl,
                            Some(SubgraphSchemaWatcherKind::Introspect(
                                specific_runner,
                                *polling_interval,
                            )),
                        )
                    }
                }
            }
            SubgraphSchemaWatcherKind::File(file_path) => {
                let sdl = Fs::read_file(file_path)?;
                (sdl, None)
            }
            SubgraphSchemaWatcherKind::Once(sdl) => (sdl.clone(), None),
        };

        let subgraph_definition = SubgraphDefinition::new(name, url, sdl);

        Ok((subgraph_definition, refresher))
    }

    fn update_subgraph(&mut self, last_message: Option<&String>) -> RoverResult<Option<String>> {
        let print_error = |e: RoverError| {
            let _ = e.print();
        };

        let maybe_update_message = match self.get_subgraph_definition_and_maybe_new_runner() {
            Ok((subgraph_definition, maybe_new_refresher)) => {
                if let Some(new_refresher) = maybe_new_refresher {
                    self.set_schema_refresher(new_refresher);
                }
                match last_message {
                    Some(last_message) => {
                        if &subgraph_definition.sdl != last_message {
                            self.message_sender.update_subgraph(&subgraph_definition)?;
                        }
                    }
                    None => {
                        self.message_sender.add_subgraph(&subgraph_definition)?;
                    }
                }
                Some(subgraph_definition.sdl)
            }
            Err(e) => {
                let error_str = e.to_string();
                match last_message {
                    Some(prev_message) => {
                        if &error_str != prev_message {
                            print_error(e);
                            self.message_sender.remove_subgraph(&self.subgraph_key.0)?;
                        }
                    }
                    None => {
                        print_error(e);
                        let _ = self.message_sender.remove_subgraph(&self.subgraph_key.0);
                    }
                }
                Some(error_str)
            }
        };

        Ok(maybe_update_message)
    }

    /// Start checking for subgraph updates and sending them to the main process.
    ///
    /// This function will block forever for `SubgraphSchemaWatcherKind` that poll for changes—so it
    /// should be started in a separate thread.
    pub fn watch_subgraph_for_changes(&mut self) -> RoverResult<()> {
        let mut last_message = None;
        match self.schema_watcher_kind.clone() {
            SubgraphSchemaWatcherKind::Introspect(introspect_runner_kind, polling_interval) => {
                let endpoint = introspect_runner_kind.endpoint();
                eprintln!(
                    "{}polling {} every {} {}",
                    Emoji::Listen,
                    &endpoint,
                    polling_interval,
                    match polling_interval {
                        1 => "second",
                        _ => "seconds",
                    }
                );
                loop {
                    last_message = self.update_subgraph(last_message.as_ref())?;
                    std::thread::sleep(std::time::Duration::from_secs(polling_interval));
                }
            }
            SubgraphSchemaWatcherKind::File(path) => {
                // populate the schema for the first time (last_message is always None to start)
                last_message = self.update_subgraph(last_message.as_ref())?;

                let (tx, rx) = unbounded();

                let watch_path = path.clone();

                Fs::watch_file(watch_path, tx);

                loop {
                    rx.recv().unwrap_or_else(|_| {
                        panic!("an unexpected error occurred while watching {}", &path)
                    });
                    last_message = self.update_subgraph(last_message.as_ref())?;
                }
            }
            SubgraphSchemaWatcherKind::Once(_) => {
                self.update_subgraph(None)?;
            }
        }
        Ok(())
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
    Introspect(IntrospectRunnerKind, u64),
    /// Watch a file on disk
    File(Utf8PathBuf),
    /// Don't ever update, schema is only pulled once
    Once(String),
}
