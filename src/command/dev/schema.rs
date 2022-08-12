use std::{net::SocketAddr, sync::mpsc::Sender};

use crate::{
    command::dev::{
        command::{CommandRunner, CommandRunnerMessage},
        netstat::{get_all_local_graphql_endpoints_except, normalize_loopback_urls},
        socket::{SubgraphKey, SubgraphName, SubgraphUrl},
        watcher::SubgraphSchemaWatcher,
        SchemaOpts,
    },
    error::RoverError,
    utils::prompt_confirm_default_yes,
    Result,
};
use dialoguer::{Input, Select};
use reqwest::blocking::Client;
use saucer::{anyhow, Fs};

impl SchemaOpts {
    pub fn get_subgraph_watcher(
        &self,
        socket_addr: &str,
        name: SubgraphName,
        command_sender: Sender<CommandRunnerMessage>,
        client: Client,
        session_subgraphs: Vec<SubgraphKey>,
    ) -> Result<SubgraphSchemaWatcher> {
        let mut preexisting_socket_addrs = Vec::new();
        for (session_subgraph_name, session_subgraph_url) in session_subgraphs {
            if let Ok(socket_addrs) = session_subgraph_url.socket_addrs(|| None) {
                preexisting_socket_addrs.extend(socket_addrs);
            }
            if session_subgraph_name == name {
                return Err(RoverError::new(anyhow!(
                    "subgraph with name '{}' is already running in this `rover dev` session",
                    &name
                )));
            } else if let Some(user_input_url) = self.subgraph_url.as_ref() {
                let normalized_user_urls = normalize_loopback_urls(user_input_url);
                let normalized_session_urls = normalize_loopback_urls(&session_subgraph_url);
                for normalized_user_url in &normalized_user_urls {
                    for normalized_session_url in &normalized_session_urls {
                        if normalized_session_url == normalized_user_url {
                            return Err(RoverError::new(anyhow!(
                                    "subgraph with url '{}' is already running in this `rover dev` session",
                                    &user_input_url
                                )));
                        }
                    }
                }
            }
        }

        let schema = if let Some(schema) = &self.subgraph_schema_path {
            Fs::assert_path_exists(schema, "")?;
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

                    if atty::is(atty::Stream::Stderr) {
                        let answer = prompt_confirm_default_yes(&format!("would you like to watch {} for changes instead of introspecting every second?", &path))?;
                        if answer {
                            Some(path)
                        } else {
                            None
                        }
                    } else {
                        eprintln!("if you would like to watch {} for changes instead of introspecting every second, pass the `--schema <PATH>` flag", &path);
                        None
                    }
                }
                _ => {
                    eprintln!("detected multiple schemas in the current working directory. you can only watch one schema at a time. to watch a schema, pass the `--schema <PATH>` flag");
                    None
                }
            }
        };

        let url = match (self.subgraph_command.as_ref(), self.subgraph_url.as_ref()) {
            // they provided a command and a url
            (Some(command), Some(url)) => {
                let (ready_sender, ready_receiver) = CommandRunner::ready_channel();
                command_sender.send(CommandRunnerMessage::SpawnTask {
                    subgraph_name: name.to_string(),
                    command: command.to_string(),
                    ready_sender,
                })?;
                ready_receiver.recv()?;
                url.clone()
            }

            // they provided a command but no url
            (Some(command), None) => {
                let (url_sender, url_receiver) = CommandRunner::url_channel();

                command_sender.send(CommandRunnerMessage::SpawnTaskAndFindUrl {
                    subgraph_name: name.to_string(),
                    command: command.to_string(),
                    client: client.clone(),
                    preexisting_socket_addrs,
                    url_sender,
                })?;
                url_receiver.recv()??
            }

            // they provided a url but no command
            (None, Some(url)) => url.clone(),

            // they did not provide a url or a command
            (None, None) => {
                let graphql_endpoints = get_all_local_graphql_endpoints_except(
                    client.clone(),
                    &preexisting_socket_addrs,
                );

                match graphql_endpoints.len() {
                    0 => {
                        eprintln!("could not detect any running GraphQL servers.");
                        ask_and_spawn_command(
                            name.to_string(),
                            command_sender,
                            client.clone(),
                            preexisting_socket_addrs,
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
                                command_sender,
                                client.clone(),
                                preexisting_socket_addrs,
                            )?
                        }
                    }
                }
            }
        };

        if let Some(schema) = schema {
            SubgraphSchemaWatcher::new_from_file_path(socket_addr, (name, url), schema)
        } else {
            SubgraphSchemaWatcher::new_from_url(socket_addr, (name, url), client)
        }
    }
}

fn ask_and_spawn_command(
    subgraph_name: SubgraphName,
    command_sender: Sender<CommandRunnerMessage>,
    client: Client,
    preexisting_socket_addrs: Vec<SocketAddr>,
) -> Result<SubgraphUrl> {
    if atty::is(atty::Stream::Stderr) {
        let command: String = Input::new()
            .with_prompt("what command do you use to start your graph?")
            .interact_text()?;
        let (url_sender, url_receiver) = CommandRunner::url_channel();

        command_sender.send(CommandRunnerMessage::SpawnTaskAndFindUrl {
            subgraph_name,
            command,
            client,
            preexisting_socket_addrs,
            url_sender,
        })?;
        url_receiver.recv()?
    } else {
        Err(RoverError::new(anyhow!("you must either pass the `--url <SUBGRAPH_URL>` argument or the `--command <SUBGRAPH_COMMAND>` argument")))
    }
}
