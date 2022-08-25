use crate::{
    command::dev::{
        netstat::normalize_loopback_urls,
        socket::{SubgraphKey, SubgraphName},
        watcher::SubgraphSchemaWatcher,
        SchemaOpts,
    },
    error::RoverError,
    utils::prompt_confirm_default_yes,
    Result,
};
use dialoguer::Input;
use reqwest::blocking::Client;
use saucer::{anyhow, Fs};

impl SchemaOpts {
    pub fn get_subgraph_watcher(
        &self,
        socket_addr: &str,
        name: SubgraphName,
        client: Client,
        session_subgraphs: Vec<SubgraphKey>,
        is_main_session: bool,
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

        let url = if let Some(subgraph_url) = &self.subgraph_url {
            subgraph_url.clone()
        } else {
            let input: String = Input::new()
                .with_prompt("what URL is your subgraph running on?")
                .interact_text()?;
            input.parse()?
        };

        if let Some(schema) = schema {
            SubgraphSchemaWatcher::new_from_file_path(
                socket_addr,
                (name, url),
                schema,
                is_main_session,
            )
        } else {
            SubgraphSchemaWatcher::new_from_url(socket_addr, (name, url), client, is_main_session)
        }
    }
}
