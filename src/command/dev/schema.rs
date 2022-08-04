use std::{sync::mpsc::channel, time::Duration};

use crate::{
    command::dev::{
        command::CommandRunner,
        introspect::{IntrospectRunnerKind, UnknownIntrospectRunner},
        netstat::get_all_local_graphql_endpoints_except,
        socket::{MessageSender, SubgraphKey, SubgraphName},
        SchemaOpts,
    },
    error::RoverError,
    Result,
};
use apollo_federation_types::build::SubgraphDefinition;
use dialoguer::{Input, Select};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use reqwest::{blocking::Client, Url};
use saucer::{anyhow, Fs, Utf8Path, Utf8PathBuf};

impl SchemaOpts {
    pub fn get_subgraph_refresher(
        &self,
        socket_addr: &str,
        name: SubgraphName,
        command_runner: &mut CommandRunner,
        client: Client,
        existing_subgraphs: Vec<Url>,
    ) -> Result<SubgraphRefresher> {
        let url = match (self.command.as_ref(), self.url.as_ref()) {
            // they provided a command and a url
            (Some(command), Some(url)) => {
                command_runner.spawn(name.to_string(), command.to_string())?;
                url.clone()
            }

            // they provided a command but no url
            (Some(command), None) => command_runner.spawn_and_find_url(
                name.to_string(),
                command.to_string(),
                client.clone(),
                &existing_subgraphs,
            )?,

            // they provided a url but no command
            (None, Some(url)) => url.clone(),

            // they did not provide a url or a command
            (None, None) => {
                eprintln!("searching for running GraphQL servers...");
                let graphql_endpoints =
                    get_all_local_graphql_endpoints_except(client.clone(), &existing_subgraphs);

                match graphql_endpoints.len() {
                    0 => {
                        eprintln!("could not detect any running GraphQL servers.");
                        ask_and_spawn_command(
                            name.to_string(),
                            command_runner,
                            client.clone(),
                            &existing_subgraphs,
                        )?
                    }
                    1 => {
                        eprintln!(
                            "detected a running GraphQL server at {}",
                            &graphql_endpoints[0]
                        );
                        graphql_endpoints[0].clone()
                    }
                    num_endpoints => {
                        eprintln!("detected {} running GraphQL servers", num_endpoints);

                        if let Ok(endpoint_index) = Select::new()
                            .items(&graphql_endpoints)
                            .default(0)
                            .interact()
                        {
                            graphql_endpoints[endpoint_index].clone()
                        } else {
                            eprintln!("could not select a GraphQL server.");
                            ask_and_spawn_command(
                                name.to_string(),
                                command_runner,
                                client.clone(),
                                &existing_subgraphs,
                            )?
                        }
                    }
                }
            }
        };

        let schema = if let Some(schema) = &self.schema {
            Some(schema.clone())
        } else {
            let mut possible_schemas = Vec::new();
            if let Ok(entries) = Fs::get_dir_entries("./", "") {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_file() {
                            if let Some(extension) = entry.path().extension() {
                                if extension == "graphql" || extension == "gql" {
                                    possible_schemas.push(entry.path().to_path_buf());
                                }
                            }
                        }
                    }
                }
            }
            match possible_schemas.len() {
                0 => {
                    eprintln!("could not detect a schema in the current working directory. to watch a schema, pass the `--schema <PATH>` flag");
                    None
                }
                1 => {
                    let path = possible_schemas[0].clone();
                    Some(path)
                }
                _ => {
                    eprintln!("detected multiple schemas in the current working directory. you can only watch one schema at a time. to watch a schema, pass the `--schema <PATH>` flag");
                    None
                }
            }
        };

        if let Some(schema) = schema {
            Ok(SubgraphRefresher::new_from_file_path(
                socket_addr,
                (name, url),
                schema,
            ))
        } else {
            Ok(SubgraphRefresher::new_from_url(
                socket_addr,
                (name, url),
                client,
            ))
        }
    }
}

fn ask_and_spawn_command(
    subgraph_name: SubgraphName,
    command_runner: &mut CommandRunner,
    client: Client,
    existing_subgraphs: &Vec<Url>,
) -> Result<Url> {
    let command: String = Input::new()
        .with_prompt("what command do you use to start your graph?")
        .interact_text()?;
    let url =
        command_runner.spawn_and_find_url(subgraph_name, command, client, existing_subgraphs)?;
    Ok(url)
}

pub struct SubgraphRefresher {
    schema_refresher: SchemaRefresherKind,
    subgraph_key: SubgraphKey,
    message_sender: MessageSender,
}

impl SubgraphRefresher {
    pub fn new_from_file_path<P>(socket_addr: &str, subgraph_key: SubgraphKey, path: P) -> Self
    where
        P: AsRef<Utf8Path>,
    {
        Self {
            schema_refresher: SchemaRefresherKind::File(path.as_ref().to_path_buf()),
            subgraph_key,
            message_sender: MessageSender::new(socket_addr),
        }
    }

    pub fn new_from_url(socket_addr: &str, subgraph_key: SubgraphKey, client: Client) -> Self {
        let (_, url) = subgraph_key.clone();
        let introspect_runner =
            IntrospectRunnerKind::Unknown(UnknownIntrospectRunner::new(url, client));
        Self::new_from_introspect_runner(socket_addr, subgraph_key, introspect_runner)
    }

    pub fn new_from_introspect_runner(
        socket_addr: &str,
        subgraph_key: SubgraphKey,
        introspect_runner: IntrospectRunnerKind,
    ) -> Self {
        Self {
            schema_refresher: SchemaRefresherKind::Introspect(introspect_runner),
            subgraph_key,
            message_sender: MessageSender::new(socket_addr),
        }
    }

    pub fn get_subgraph_definition_and_maybe_new_runner(
        &self,
    ) -> Result<(SubgraphDefinition, Option<SchemaRefresherKind>)> {
        let (name, url) = self.subgraph_key.clone();
        let (sdl, refresher) = match &self.schema_refresher {
            SchemaRefresherKind::Introspect(introspect_runner_kind) => match introspect_runner_kind
            {
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
                    (sdl, Some(SchemaRefresherKind::Introspect(specific_runner)))
                }
            },
            SchemaRefresherKind::File(file_path) => {
                let sdl = Fs::read_file(file_path, "")?;
                (sdl, None)
            }
        };

        let subgraph_definition = SubgraphDefinition::new(name.to_string(), url.clone(), sdl);

        Ok((subgraph_definition, refresher))
    }

    fn update_subgraph(&mut self, last_message: Option<&String>) -> Result<Option<String>> {
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
                        Some(subgraph_definition.sdl.to_string())
                    }
                    None => {
                        self.message_sender.add_subgraph(&subgraph_definition)?;
                        Some(subgraph_definition.sdl.to_string())
                    }
                }
            }
            Err(e) => {
                let error_str = e.to_string();
                match last_message {
                    Some(prev_message) => {
                        if &error_str != prev_message {
                            let _ = e.print();
                        }
                        Some(error_str)
                    }
                    None => {
                        let _ = e.print();
                        Some(error_str)
                    }
                }
            }
        };

        Ok(maybe_update_message)
    }

    pub fn watch_subgraph(&mut self) -> Result<()> {
        let mut last_message = None;
        match &self.schema_refresher {
            SchemaRefresherKind::Introspect(_) => loop {
                last_message = self.update_subgraph(last_message.as_ref())?;
                std::thread::sleep(std::time::Duration::from_secs(1));
            },
            SchemaRefresherKind::File(path) => {
                let path = path.to_string();
                eprintln!("watching {} for changes", &path);
                let (broadcaster, listener) = channel();
                let mut watcher = watcher(broadcaster, Duration::from_secs(1))?;
                watcher.watch(&path, RecursiveMode::NonRecursive)?;

                last_message = self.update_subgraph(last_message.as_ref())?;

                loop {
                    match listener.recv() {
                        Ok(event) => match &event {
                            DebouncedEvent::NoticeWrite(_) => {
                                eprintln!("change detected in {}", &path);
                            }
                            DebouncedEvent::Write(_) => {
                                eprintln!("updating subgraph from watched file...");
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

    pub fn set_schema_refresher(&mut self, new_refresher: SchemaRefresherKind) {
        self.schema_refresher = new_refresher;
    }
}

#[derive(Debug, Clone)]
pub enum SchemaRefresherKind {
    Introspect(IntrospectRunnerKind),
    File(Utf8PathBuf),
}
