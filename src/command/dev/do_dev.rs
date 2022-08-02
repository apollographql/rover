use std::io::{self, prelude::*, BufReader};
use std::time::Duration;

use apollo_federation_types::build::SubgraphDefinition;
use dialoguer::Input;
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use saucer::{anyhow, clap, Context, Parser, Utf8PathBuf};
use serde::Serialize;
use tempdir::TempDir;

use super::command::CommandRunner;
use super::compose::ComposeRunner;
use super::router::RouterRunner;
use super::{Dev, DevOpts};
use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

impl DevOpts {
    pub fn get_name(&self) -> Result<String> {
        if let Some(name) = self.subgraph_name.as_ref().map(|s| s.to_string()) {
            Ok(name)
        } else {
            let dirname = std::env::current_dir().ok().and_then(|x| {
                x.file_name()
                    .and_then(|x| Some(x.to_string_lossy().to_string()))
            });
            let mut input = Input::new();
            input.with_prompt("what is the name of this subgraph?");
            if let Some(dirname) = dirname {
                input.default(dirname);
            }
            let name: String = input.interact_text()?;
            Ok(name)
        }
    }
}

impl Dev {
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        let mut command_runner = CommandRunner::new();

        let socket_addr = "/tmp/supergraph.sock";

        let existing_subgraphs =
            if let Ok(subgraph_stream) = LocalSocketStream::connect(socket_addr) {
                let mut reader = BufReader::new(subgraph_stream);
                let mut buffer = String::new();
                reader.read_line(&mut buffer)?;
                let existing_subgraphs: Vec<SubgraphDefinition> =
                    serde_json::from_str(&buffer).expect("could not get existing subgraphs");
                existing_subgraphs
            } else {
                Vec::new()
            };

        let schema_refresher = self.opts.schema_opts.get_schema_refresher(
            &mut command_runner,
            client_config.get_reqwest_client(),
            &existing_subgraphs,
        )?;

        let sdl = schema_refresher.get_sdl()?;
        let url = schema_refresher.get_url();
        let name = self.opts.get_name()?;

        let this_subgraph = SubgraphDefinition::new(&name, url.clone(), sdl);
        let this_subgraph_json = serde_json::to_string(&this_subgraph)
            .with_context(|| format!("could not convert {} to JSON", &name))?;

        let temp_dir = TempDir::new("subgraph")?;
        let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?.join("supergraph.graphql");

        if let Ok(mut subgraph_stream) = LocalSocketStream::connect(socket_addr) {
            eprintln!(
                "a `rover dev` sesssion is already running on this computer with {} subgraphs, extending it..."
            , existing_subgraphs.len());
            subgraph_stream
                .write_all(format!("{}\n", this_subgraph_json).as_bytes())
                .context("could not inform other `rover dev` session about your subgraph")?;

            let mut conn = BufReader::new(subgraph_stream);
            let mut buffer = String::new();
            conn.read_line(&mut buffer)?;
            eprintln!("{}", buffer);
            command_runner.wait();
        } else {
            eprintln!("no `rover dev` session is running, starting a supergraph from scratch...");
            let _ = std::fs::remove_file(&socket_addr);
            let mut compose_runner = ComposeRunner::new(
                self.opts.plugin_opts.clone(),
                override_install_path.clone(),
                client_config.clone(),
                vec![this_subgraph],
                temp_path.clone(),
            );

            compose_runner.run()?;

            let subgraph_listener = LocalSocketListener::bind(socket_addr).with_context(|| {
                format!("could not start local socket server at {}", socket_addr)
            })?;

            let router_runner = RouterRunner::new(
                temp_path,
                self.opts.plugin_opts.clone(),
                override_install_path,
                client_config,
            );
            command_runner.spawn(router_runner.get_command_to_spawn()?)?;
            // TODO: replace this with something that polls a health check on the router
            std::thread::sleep(Duration::from_millis(500));
            eprintln!("router is running! head to http://localhost:4000 to query your supergraph");
            for mut incoming_connection in
                subgraph_listener.incoming().filter_map(handle_socket_error)
            {
                eprintln!("informing new session of exsiting endpoints");
                incoming_connection
                    .write_all(compose_runner.taken_endpoints_message().as_bytes())?;
                eprintln!("successfully informed new session of endpoints");
                let mut connection_reader = BufReader::new(incoming_connection);
                let mut subgraph_definition_buffer = String::new();
                match connection_reader.read_line(&mut subgraph_definition_buffer) {
                    Ok(_) => {
                        match serde_json::from_str::<SubgraphDefinition>(
                            &subgraph_definition_buffer,
                        ) {
                            Ok(subgraph_definition) => {
                                compose_runner.add_subgraph(subgraph_definition)?;
                            }
                            Err(_) => {
                                eprintln!(
                                    "incoming message was not a valid subgraph:\n{}",
                                    &subgraph_definition_buffer
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("could not read incoming line from socket stream. {}", e);
                    }
                }
            }
        }
        Ok(RoverOutput::EmptySuccess)
    }
}

fn handle_socket_error(conn: io::Result<LocalSocketStream>) -> Option<LocalSocketStream> {
    match conn {
        Ok(val) => Some(val),
        Err(error) => {
            eprintln!("Incoming connection failed: {}", error);
            None
        }
    }
}
