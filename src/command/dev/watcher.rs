use std::{sync::mpsc::channel, time::Duration};

use crate::{
    command::dev::{
        follower::FollowerMessenger,
        introspect::{IntrospectRunnerKind, UnknownIntrospectRunner},
        protocol::SubgraphKey,
    },
    error::RoverError,
    Result,
};
use apollo_federation_types::build::SubgraphDefinition;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use reqwest::blocking::Client;
use saucer::{anyhow, Fs, Utf8Path, Utf8PathBuf};

pub struct SubgraphSchemaWatcher {
    schema_watcher_kind: SubgraphSchemaWatcherKind,
    subgraph_key: SubgraphKey,
    message_sender: FollowerMessenger,
}

impl SubgraphSchemaWatcher {
    pub fn new_from_file_path<P>(
        socket_addr: &str,
        subgraph_key: SubgraphKey,
        path: P,
        is_main_session: bool,
    ) -> Result<Self>
    where
        P: AsRef<Utf8Path>,
    {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::File(path.as_ref().to_path_buf()),
            subgraph_key,
            message_sender: FollowerMessenger::new(socket_addr, is_main_session),
        })
    }

    pub fn new_from_url(
        socket_addr: &str,
        subgraph_key: SubgraphKey,
        client: Client,
        is_main_session: bool,
    ) -> Result<Self> {
        let (_, url) = subgraph_key.clone();
        let introspect_runner =
            IntrospectRunnerKind::Unknown(UnknownIntrospectRunner::new(url, client));
        Self::new_from_introspect_runner(
            socket_addr,
            subgraph_key,
            introspect_runner,
            is_main_session,
        )
    }

    pub fn new_from_introspect_runner(
        socket_addr: &str,
        subgraph_key: SubgraphKey,
        introspect_runner: IntrospectRunnerKind,
        is_main_session: bool,
    ) -> Result<Self> {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::Introspect(introspect_runner),
            subgraph_key,
            message_sender: FollowerMessenger::new(socket_addr, is_main_session),
        })
    }

    pub fn get_subgraph_definition_and_maybe_new_runner(
        &self,
    ) -> Result<(SubgraphDefinition, Option<SubgraphSchemaWatcherKind>)> {
        let (name, url) = self.subgraph_key.clone();
        let (sdl, refresher) = match &self.schema_watcher_kind {
            SubgraphSchemaWatcherKind::Introspect(introspect_runner_kind) => {
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
                            Some(SubgraphSchemaWatcherKind::Introspect(specific_runner)),
                        )
                    }
                }
            }
            SubgraphSchemaWatcherKind::File(file_path) => {
                let sdl = Fs::read_file(file_path, "")?;
                (sdl, None)
            }
        };

        let subgraph_definition = SubgraphDefinition::new(name, url, sdl);

        Ok((subgraph_definition, refresher))
    }

    fn update_subgraph(&mut self, last_message: Option<&String>) -> Result<Option<String>> {
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
                        }
                    }
                    None => {
                        print_error(e);
                    }
                }
                Some(error_str)
            }
        };

        Ok(maybe_update_message)
    }

    pub fn watch_subgraph(&mut self) -> Result<()> {
        eprintln!(
            "watching '{}' subgraph for changes...",
            &self.subgraph_key.0
        );
        let mut last_message = None;
        match &self.schema_watcher_kind {
            SubgraphSchemaWatcherKind::Introspect(introspect_runner_kind) => {
                let poll_interval_secs = 1;
                let endpoint = introspect_runner_kind.endpoint();
                eprintln!(
                    "polling {} every {} {}",
                    &endpoint,
                    poll_interval_secs,
                    match poll_interval_secs {
                        1 => "second",
                        _ => "seconds",
                    }
                );
                loop {
                    last_message = self.update_subgraph(last_message.as_ref())?;
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
            SubgraphSchemaWatcherKind::File(path) => {
                let path = path.to_string();
                last_message = self.update_subgraph(last_message.as_ref())?;
                eprintln!("watching {} for changes", &path);
                let (broadcaster, listener) = channel();
                let mut watcher = watcher(broadcaster, Duration::from_secs(1))?;
                watcher.watch(&path, RecursiveMode::NonRecursive)?;

                loop {
                    match listener.recv() {
                        Ok(event) => match &event {
                            DebouncedEvent::NoticeWrite(_) => {
                                eprintln!("change detected in {}...", &path);
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

    pub fn is_main_session(&self) -> bool {
        self.message_sender.is_main_session()
    }
}

#[derive(Debug, Clone)]
pub enum SubgraphSchemaWatcherKind {
    Introspect(IntrospectRunnerKind),
    File(Utf8PathBuf),
}
