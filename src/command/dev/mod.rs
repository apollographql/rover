mod command;
mod compose;
mod introspect;
mod router;

use command::CommandRunner;
use compose::ComposeRunner;
use introspect::IntrospectRunner;
use router::RouterRunner;

use std::io::{self, prelude::*, BufReader};
use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

use apollo_federation_types::build::SubgraphDefinition;
use dialoguer::Input;
use dialoguer::Select;
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use netstat2::*;
use saucer::{anyhow, clap, Context, Parser, Saucer, Utf8PathBuf};
use serde::Serialize;
use tempdir::TempDir;

use crate::command::RoverOutput;
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::{error::RoverError, Result};

#[derive(Debug, Serialize, Parser)]
pub struct Dev {
    #[clap(flatten)]
    pub(crate) opts: DevOpts,
}

#[derive(Debug, Clone, Serialize, Parser)]
pub struct DevOpts {
    #[clap(flatten)]
    plugin_opts: PluginOpts,

    /// Url of a running subgraph that a graph router can send operations to
    /// (often a localhost endpoint).
    #[clap(long)]
    #[serde(skip_serializing)]
    server_url: Option<String>,

    #[clap(long)]
    debug_socket: Option<String>,
}

impl Dev {
    #[cfg(feature = "composition-js")]
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        let mut command_runner = CommandRunner::new();

        let socket_addr = "/tmp/supergraph.sock";
        if let Some(message) = &self.opts.debug_socket {
            if let Ok(mut subgraph_stream) = LocalSocketStream::connect(socket_addr) {
                eprintln!("connected to existing rover dev instance");
                subgraph_stream.write_all(format!("{}\n", &message).as_bytes())?;
                let mut incoming = BufReader::new(subgraph_stream);
                let mut incoming_buffer = String::new();
                if incoming.read_line(&mut incoming_buffer).is_ok() {
                    eprintln!("{}", &incoming_buffer);
                }
                Ok(RoverOutput::EmptySuccess)
            } else {
                Err(RoverError::new(anyhow!(
                    "couldn't connect to the socket, run `rover dev` first"
                )))
            }
        } else {
            let maybe_endpoint = if let Some(server_url) = &self.opts.server_url {
                Some(server_url.to_string())
            } else {
                eprintln!("it looks like this directory does not have any configuration...");
                eprintln!("walking through setup steps now...");
                let input: String = Input::new()
                    .with_prompt("Is your GraphQL server already running? [y/N]")
                    .default("no".into())
                    .show_default(false)
                    .interact_text()?;

                if input.to_lowercase().starts_with('y') {
                    None
                } else {
                    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
                    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;
                    let mut pre_existing_ports = Vec::new();
                    if let Ok(sockets_info) = get_sockets_info(af_flags, proto_flags) {
                        for si in &sockets_info {
                            if &si.local_addr().to_string() == "::" {
                                pre_existing_ports.push(si.local_port());
                            }
                        }
                    }

                    let command: String = Input::new()
                        .with_prompt("what command do you use to start your graph?")
                        .interact_text()?;

                    command_runner.spawn(command.to_string())?;

                    let mut possible_ports = Vec::new();
                    let now = Instant::now();
                    while possible_ports.is_empty() && now.elapsed() < Duration::from_secs(10) {
                        std::thread::sleep(Duration::from_millis(500));
                        for si in get_sockets_info(af_flags, proto_flags)? {
                            if &si.local_addr().to_string() == "::" {
                                let port = si.local_port();
                                if !pre_existing_ports.contains(&port) {
                                    possible_ports.push(port);
                                }
                            }
                        }
                    }

                    if possible_ports.is_empty() {
                        eprintln!("warn: it looks like we didn't detect an endpoint from that command. if you think it's running you can enter the endpoint now. otherwise, press ctrl+c and debug `{}`", &command);
                    }

                    let maybe_port = match possible_ports.len() {
                        0 => None,
                        1 => Some(possible_ports[0]),
                        _ => {
                            if let Ok(endpoint_index) =
                                Select::new().items(&possible_ports).default(0).interact()
                            {
                                Some(possible_ports[endpoint_index])
                            } else {
                                None
                            }
                        }
                    };

                    maybe_port.map(|p| format!("http://localhost:{}", &p))
                }
            };

            let endpoint: reqwest::Url = if let Some(endpoint) = maybe_endpoint {
                eprintln!("detected endpoint {}", &endpoint);
                endpoint
            } else {
                let endpoint = Input::new()
                    .with_prompt("what endpoint is your graph running on?")
                    .interact_text()?;
                endpoint
            }
            .parse()?;

            let (sdl_sender, sdl_receiver) = sync_channel(1);
            let introspect_runner = IntrospectRunner::new(
                endpoint.clone(),
                sdl_sender,
                client_config.get_reqwest_client(),
            );

            eprintln!("introspecting {}", &endpoint);
            introspect_runner.beam()?;

            let sdl = sdl_receiver.recv()??;
            let this_subgraph_name = std::env::current_dir()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            eprintln!("using dir name {} as subgraph name", &this_subgraph_name);
            let this_subgraph = SubgraphDefinition::new(&this_subgraph_name, endpoint, sdl);
            let this_subgraph_json = serde_json::to_string(&this_subgraph)
                .with_context(|| format!("could not convert {} to JSON", &this_subgraph_name))?;

            let temp_dir = TempDir::new("subgraph")?;
            let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?.join("supergraph.graphql");

            if let Ok(mut subgraph_stream) = LocalSocketStream::connect(socket_addr) {
                eprintln!(
                    "a `rover dev` sesssion is already running on this computer, extending it..."
                );
                subgraph_stream
                    .write_all(format!("{}\n", this_subgraph_json).as_bytes())
                    .context("could not inform other `rover dev` session about your subgraph")?;
                let mut conn = BufReader::new(subgraph_stream);
                let mut buffer = String::new();
                conn.read_line(&mut buffer)?;
                eprintln!("{}", buffer);
                // sleep forever because the user's command is running in the background
                loop {
                    std::thread::sleep(Duration::MAX)
                }
            } else {
                eprintln!(
                    "no `rover dev` session is running, starting a supergraph from scratch..."
                );
                let _ = std::fs::remove_file(&socket_addr);
                let mut compose_saucer = ComposeRunner::new(
                    self.opts.plugin_opts.clone(),
                    override_install_path.clone(),
                    client_config.clone(),
                    vec![this_subgraph],
                    temp_path.clone(),
                );

                compose_saucer.run()?;

                let subgraph_listener =
                    LocalSocketListener::bind(socket_addr).with_context(|| {
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
                eprintln!(
                    "router is running! head to http://localhost:4000 to query your supergraph"
                );
                for mut incoming_connection in
                    subgraph_listener.incoming().filter_map(handle_socket_error)
                {
                    incoming_connection.write_all(
                        "successfully added subgraph to rover dev session\n".as_bytes(),
                    )?;
                    let mut connection_reader = BufReader::new(incoming_connection);
                    let mut subgraph_definition_buffer = String::new();
                    match connection_reader.read_line(&mut subgraph_definition_buffer) {
                        Ok(_) => {
                            match serde_json::from_str::<SubgraphDefinition>(
                                &subgraph_definition_buffer,
                            ) {
                                Ok(subgraph_definition) => {
                                    compose_saucer.add_subgraph(subgraph_definition)?;
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

    #[cfg(not(feature = "composition-js"))]
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        Err(RoverError::new(anyhow!(
            "rover dev is not supported on this platform"
        )))
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
