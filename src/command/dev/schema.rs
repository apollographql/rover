use crate::{
    command::dev::{
        command::CommandRunner, netstat::get_all_local_graphql_endpoints_except,
        socket::SubgraphName, watcher::SubgraphSchemaWatcher, SchemaOpts,
    },
    Result,
};
use dialoguer::{Input, Select};
use reqwest::{blocking::Client, Url};
use saucer::Fs;

impl SchemaOpts {
    pub fn get_subgraph_watcher(
        &self,
        socket_addr: &str,
        name: SubgraphName,
        command_runner: &mut CommandRunner,
        client: Client,
        existing_subgraphs: Vec<Url>,
    ) -> Result<SubgraphSchemaWatcher> {
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
            Ok(SubgraphSchemaWatcher::new_from_file_path(
                socket_addr,
                (name, url),
                schema,
            ))
        } else {
            Ok(SubgraphSchemaWatcher::new_from_url(
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
    existing_subgraphs: &[Url],
) -> Result<Url> {
    let command: String = Input::new()
        .with_prompt("what command do you use to start your graph?")
        .interact_text()?;
    let url =
        command_runner.spawn_and_find_url(subgraph_name, command, client, existing_subgraphs)?;
    Ok(url)
}
