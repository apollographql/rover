use crate::{
    command::dev::{
        command::CommandRunner, introspect::IntrospectRunner,
        netstat::get_all_local_graphql_endpoints_except, DevOpts, SchemaOpts,
    },
    Result,
};
use apollo_federation_types::build::SubgraphDefinition;
use dialoguer::{Input, Select};
use reqwest::{blocking::Client, Url};
use saucer::{anyhow, clap, Fs, Parser, Utf8Path, Utf8PathBuf};
use serde::Serialize;

impl SchemaOpts {
    pub fn get_schema_refresher(
        &self,
        command_runner: &mut CommandRunner,
        client: Client,
        existing_subgraphs: &Vec<SubgraphDefinition>,
    ) -> Result<SchemaRefresher> {
        let url = match (self.subgraph_command.as_ref(), self.subgraph_url.as_ref()) {
            // they provided a command and a url
            (Some(command), Some(url)) => {
                command_runner.spawn(command.to_string())?;
                url.clone()
            }

            // they provided a command but no url
            (Some(command), None) => {
                let url = command_runner.spawn_and_find_url(
                    command.to_string(),
                    client.clone(),
                    existing_subgraphs,
                )?;
                url
            }

            // they provided a url but no command
            (None, Some(url)) => url.clone(),

            // they did not provide a url or a command
            (None, None) => {
                eprintln!("searching for running GraphQL servers...");
                let graphql_endpoints = get_all_local_graphql_endpoints_except(
                    client.clone(),
                    &existing_subgraphs
                        .iter()
                        .filter_map(|s| Url::parse(&s.url).ok())
                        .collect(),
                );

                match graphql_endpoints.len() {
                    0 => {
                        eprintln!("could not detect any running GraphQL servers.");
                        let url = ask_and_spawn_command(
                            command_runner,
                            client.clone(),
                            existing_subgraphs,
                        )?;
                        url
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
                            let url = ask_and_spawn_command(
                                command_runner,
                                client.clone(),
                                existing_subgraphs,
                            )?;
                            url
                        }
                    }
                }
            }
        };

        for existing_subgraph in existing_subgraphs {
            if existing_subgraph.url == url.to_string() {
                return Err(anyhow!("this `rover dev` session already includes subgraph '{}' which is running on '{}'", &existing_subgraph.name, &existing_subgraph.url).into());
            }
        }

        if let Some(subgraph_schema) = &self.subgraph_schema {
            Ok(SchemaRefresher::new_from_file_path(
                url.clone(),
                subgraph_schema.clone(),
            ))
        } else {
            Ok(SchemaRefresher::new_from_url(url.clone(), client.clone()))
        }
    }
}

fn ask_and_spawn_command(
    command_runner: &mut CommandRunner,
    client: Client,
    existing_subgraphs: &Vec<SubgraphDefinition>,
) -> Result<Url> {
    let command: String = Input::new()
        .with_prompt("what command do you use to start your graph?")
        .interact_text()?;
    let url = command_runner.spawn_and_find_url(command.to_string(), client, existing_subgraphs)?;
    Ok(url)
}

pub struct SchemaRefresher {
    schema: SchemaRefresherKind,
    url: Url,
}

impl SchemaRefresher {
    pub fn new_from_file_path<P>(url: Url, path: P) -> Self
    where
        P: AsRef<Utf8Path>,
    {
        Self {
            schema: SchemaRefresherKind::File(path.as_ref().to_path_buf()),
            url,
        }
    }

    pub fn new_from_url(url: Url, client: Client) -> Self {
        Self {
            schema: SchemaRefresherKind::Introspect(IntrospectRunner::new(url.clone(), client)),
            url,
        }
    }

    pub fn get_sdl(&self) -> Result<String> {
        self.schema.get_sdl()
    }

    pub fn get_url(&self) -> Url {
        self.url.clone()
    }
}

enum SchemaRefresherKind {
    Introspect(IntrospectRunner),
    File(Utf8PathBuf),
}

impl SchemaRefresherKind {
    pub fn get_sdl(&self) -> Result<String> {
        match &self {
            Self::Introspect(introspect_runner) => introspect_runner.run(),
            Self::File(file_path) => Ok(Fs::read_file(&file_path, "")?),
        }
    }
}
