use std::{sync::mpsc::channel, time::Duration};

use crate::{
    command::dev::{
        introspect::{IntrospectRunnerKind, UnknownIntrospectRunner},
        protocol::{FollowerMessenger, SubgraphKey},
    },
    RoverError, RoverResult,
};

use anyhow::anyhow;
use apollo_federation_types::build::SubgraphDefinition;
use camino::{Utf8Path, Utf8PathBuf};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use reqwest::blocking::Client;
use rover_std::{Emoji, Fs};

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
    ) -> RoverResult<Self> {
        let (_, url) = subgraph_key.clone();
        let introspect_runner =
            IntrospectRunnerKind::Unknown(UnknownIntrospectRunner::new(url, client));
        Self::new_from_introspect_runner(
            subgraph_key,
            introspect_runner,
            message_sender,
            polling_interval,
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

    pub fn watch_subgraph_for_changes(&mut self) -> RoverResult<()> {
        let mut last_message = None;
        match &self.schema_watcher_kind {
            SubgraphSchemaWatcherKind::Introspect(introspect_runner_kind, polling_interval) => {
                let endpoint = introspect_runner_kind.endpoint();
                let polling_interval = *polling_interval;
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
                let path = path.to_string();
                last_message = self.update_subgraph(last_message.as_ref())?;
                eprintln!("{}watching {} for changes", Emoji::Watch, &path);
                let (broadcaster, listener) = channel();
                let mut watcher = watcher(broadcaster, Duration::from_secs(1))?;
                watcher.watch(&path, RecursiveMode::NonRecursive)?;

                loop {
                    match listener.recv() {
                        Ok(event) => match &event {
                            DebouncedEvent::NoticeWrite(_) => {
                                eprintln!("{}change detected in {}...", Emoji::Sparkle, &path);
                            }
                            DebouncedEvent::Write(_) => {
                                last_message = self.update_subgraph(last_message.as_ref())?;
                            }
                            _ => {}
                        },
                        Err(e) => {
                            let _ = RoverError::new(anyhow!("{}", e)).print();
                        }
                    };
                }
            }
        };
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
    Introspect(IntrospectRunnerKind, u64),
    File(Utf8PathBuf),
}
