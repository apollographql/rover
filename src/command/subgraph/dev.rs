use std::borrow::Borrow;
use std::io::{self, prelude::*, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::time::Duration;

use apollo_federation_types::build::SubgraphDefinition;
use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use dialoguer::Input;
use dialoguer::Select;
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use netstat2::*;
use reqwest::blocking::Client;
use saucer::{anyhow, clap, Context, Fs, ParallelSaucer, Parser, Saucer, Utf8PathBuf};
use serde::Serialize;
use tempdir::TempDir;

use crate::command::install::Plugin;
use crate::command::subgraph::introspect::Introspect;
use crate::command::supergraph::compose::Compose;
use crate::command::{Install, RoverOutput};
use crate::options::{OptionalGraphRefOpt, OptionalSchemaOpt, OptionalSubgraphOpt, PluginOpts};
use crate::utils::client::StudioClientConfig;
use crate::{error::RoverError, Result};

#[derive(Debug, Serialize, Parser)]
pub struct Dev {
    #[clap(flatten)]
    pub(crate) opts: SubgraphDevOpts,
}

#[derive(Debug, Clone, Serialize, Parser)]
pub struct SubgraphDevOpts {
    #[clap(flatten)]
    graph: OptionalGraphRefOpt,

    #[clap(flatten)]
    subgraph: OptionalSubgraphOpt,

    #[clap(flatten)]
    plugin_opts: PluginOpts,

    #[clap(flatten)]
    #[serde(skip_serializing)]
    schema: OptionalSchemaOpt,

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
        use std::{borrow::BorrowMut, time::Instant};
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

                if input.to_lowercase().starts_with("y") {
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
                        1 => Some(possible_ports[0].clone()),
                        _ => {
                            if let Ok(endpoint_index) =
                                Select::new().items(&possible_ports).default(0).interact()
                            {
                                Some(possible_ports[endpoint_index].clone())
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
            let introspect_saucer = IntrospectSaucer {
                endpoint: endpoint.clone(),
                sdl_sender,
                client: client_config.get_reqwest_client(),
            };

            eprintln!("introspecting {}", &endpoint);
            introspect_saucer.beam()?;

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
                // this loop is here so that we don't kill the command too soon
                loop {}
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
                    incoming_connection.write(
                        format!("successfully added subgraph to rover dev session\n",).as_bytes(),
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

#[cfg(feature = "composition-js")]
#[derive(Debug, Clone)]
pub struct RouterRunner {
    read_path: Utf8PathBuf,
    opts: PluginOpts,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
}

impl RouterRunner {
    fn new(
        read_path: Utf8PathBuf,
        opts: PluginOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Self {
        Self {
            read_path,
            opts,
            override_install_path,
            client_config,
        }
    }

    fn get_command_to_spawn(&self) -> Result<String> {
        let plugin = Plugin::Router;
        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepted: self.opts.elv2_license_accepted,
        };

        // maybe do the install, maybe find a pre-existing installation, maybe fail
        let exe = install_command
            .get_versioned_plugin(
                self.override_install_path.clone(),
                self.client_config.clone(),
                self.opts.skip_update,
            )
            .map_err(|e| anyhow!("{}", e))?;

        Ok(format!(
            "{} --supergraph {} --hot-reload",
            &exe,
            self.read_path.as_str()
        ))
    }
}

#[cfg(feature = "composition-js")]
#[derive(Debug, Clone)]
pub struct ComposeRunner {
    compose: Compose,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    subgraph_definitions: Vec<SubgraphDefinition>,
    write_path: Utf8PathBuf,
}

impl ComposeRunner {
    fn new(
        compose_opts: PluginOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        subgraph_definitions: Vec<SubgraphDefinition>,
        write_path: Utf8PathBuf,
    ) -> Self {
        Self {
            compose: Compose::new(compose_opts),
            override_install_path,
            client_config,
            subgraph_definitions,
            write_path,
        }
    }

    fn add_subgraph(&mut self, subgraph_definition: SubgraphDefinition) -> Result<()> {
        self.subgraph_definitions.push(subgraph_definition);
        self.run()
    }

    fn run(&self) -> Result<()> {
        let mut supergraph_config = SupergraphConfig::from(self.subgraph_definitions.clone());
        supergraph_config.set_federation_version(FederationVersion::LatestFedTwo);
        match self.compose.compose(
            self.override_install_path.clone(),
            self.client_config.clone(),
            &mut supergraph_config.clone(),
        ) {
            Ok(build_result) => match &build_result {
                RoverOutput::CompositionResult {
                    supergraph_sdl,
                    hints: _,
                    federation_version: _,
                } => {
                    let context = format!("could not write SDL to {}", &self.write_path);
                    match std::fs::File::create(&self.write_path) {
                        Ok(mut opened_file) => {
                            if let Err(e) = opened_file.write_all(supergraph_sdl.as_bytes()) {
                                Err(RoverError::new(
                                    anyhow!("{}", e)
                                        .context("could not write bytes")
                                        .context(context),
                                ))
                            } else if let Err(e) = opened_file.flush() {
                                Err(RoverError::new(
                                    anyhow!("{}", e)
                                        .context("could not flush file")
                                        .context(context),
                                ))
                            } else {
                                eprintln!(
                                    "wrote updated supergraph schema to {}",
                                    &self.write_path
                                );
                                Ok(())
                            }
                        }
                        Err(e) => Err(RoverError::new(anyhow!("{}", e).context(context))),
                    }
                }
                _ => unreachable!(),
            },
            Err(e) => Err(anyhow!("{}", e).into()),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct IntrospectSaucer {
    endpoint: reqwest::Url,
    sdl_sender: SyncSender<saucer::Result<String>>,
    client: Client,
}

impl Saucer for IntrospectSaucer {
    fn description(&self) -> String {
        "introspect".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        let (subgraph_sender, subgraph_receiver) = sync_channel(1);
        let (graph_sender, graph_receiver) = sync_channel(1);
        // stage 1 of 1
        self.introspect(subgraph_sender, graph_sender, 1, 1)
            .beam()?;

        let graph_result = graph_receiver.recv()?;
        let subgraph_result = subgraph_receiver.recv()?;

        match (subgraph_result, graph_result) {
            (Ok(s), _) => {
                eprintln!("fetching federated SDL succeeded");
                self.sdl_sender.send(Ok(s))?;
            }
            (Err(_), Ok(s)) => {
                eprintln!("warn: could not fetch federated SDL, using introspection schema without directives. you should convert this monograph to a subgraph. see https://www.apollographql.com/docs/federation/subgraphs/ for more information.");
                self.sdl_sender.send(Ok(s))?;
            }
            (Err(se), Err(ge)) => {
                self.sdl_sender
                    .send(Err(anyhow!("could not introspect {}", &self.endpoint)
                        .context(se)
                        .context(ge)))?;
            }
        }

        Ok(())
    }
}

impl IntrospectSaucer {
    fn introspect(
        &self,
        subgraph_sender: SyncSender<Result<String>>,
        graph_sender: SyncSender<Result<String>>,
        current_stage: usize,
        total_stages: usize,
    ) -> ParallelSaucer<SubgraphIntrospectSaucer, GraphIntrospectSaucer> {
        ParallelSaucer::new(
            SubgraphIntrospectSaucer {
                sender: subgraph_sender.clone(),
                endpoint: self.endpoint.clone(),
                client: self.client.clone(),
            },
            GraphIntrospectSaucer {
                sender: graph_sender.clone(),
                endpoint: self.endpoint.clone(),
                client: self.client.clone(),
            },
            &self.prefix(),
            current_stage,
            total_stages,
        )
    }
}

#[derive(Debug, Clone)]
struct SubgraphIntrospectSaucer {
    endpoint: reqwest::Url,
    sender: SyncSender<Result<String>>,
    client: Client,
}

impl SubgraphIntrospectSaucer {
    pub fn new(endpoint: &str, sender: SyncSender<Result<String>>, client: Client) -> Result<Self> {
        Ok(Self {
            endpoint: endpoint.parse()?,
            sender,
            client,
        })
    }
}

impl Saucer for SubgraphIntrospectSaucer {
    fn description(&self) -> String {
        "subgraph introspect".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        eprintln!("running subgraph introspect");
        let output = Introspect {
            endpoint: self.endpoint.clone(),
            headers: None,
        }
        .run(self.client.clone());
        match output {
            Ok(rover_output) => match rover_output {
                RoverOutput::Introspection(sdl) => {
                    self.sender.send(Ok(sdl))?;
                }
                _ => {
                    self.sender.send(Err(
                        anyhow!("invalid result from subgraph introspect").into()
                    ))?;
                }
            },
            Err(e) => {
                self.sender.send(Err(e))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct GraphIntrospectSaucer {
    endpoint: reqwest::Url,
    sender: SyncSender<Result<String>>,
    client: Client,
}

impl GraphIntrospectSaucer {
    pub fn new(endpoint: &str, sender: SyncSender<Result<String>>, client: Client) -> Result<Self> {
        Ok(Self {
            endpoint: endpoint.parse()?,
            sender,
            client,
        })
    }
}

impl Saucer for GraphIntrospectSaucer {
    fn description(&self) -> String {
        "graph introspect".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        eprintln!("running graph introspect");
        let output = Introspect {
            endpoint: self.endpoint.clone(),
            headers: None,
        }
        .run(self.client.clone());
        match output {
            Ok(rover_output) => match rover_output {
                RoverOutput::Introspection(sdl) => {
                    self.sender.send(Ok(sdl))?;
                }
                _ => {
                    self.sender
                        .send(Err(anyhow!("invalid result from graph introspect").into()))?;
                }
            },
            Err(e) => {
                self.sender.send(Err(e))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct CommandRunner {
    tasks: Vec<BackgroundTask>,
}

impl CommandRunner {
    pub(crate) fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    fn spawn(&mut self, command: String) -> Result<()> {
        let args: Vec<&str> = command.split(" ").collect();
        let (bin, args) = match args.len() {
            0 => Err(anyhow!("the command you passed is empty")),
            1 => Ok((args[0], Vec::new())),
            _ => Ok((args[0], Vec::from_iter(args[1..].iter()))),
        }?;
        eprintln!("starting `{}`", &command);
        if which::which(bin).is_ok() {
            let mut command = Command::new(bin);
            command.args(args);
            self.tasks.push(BackgroundTask::new(command)?);
            Ok(())
        } else {
            Err(anyhow!("{} is not installed on this machine", &bin).into())
        }
    }

    // no-op that ensures [`BackgroundTask`]s aren't killed prematurely
    fn join(&self) -> Result<()> {
        eprintln!("dropping {} tasks", self.tasks.len());
        Ok(())
    }
}

#[derive(Debug)]
pub struct BackgroundTask {
    pub child: Child,
}

impl BackgroundTask {
    pub fn new(mut command: Command) -> Result<Self> {
        if cfg!(windows) {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        eprintln!("spawning {:?}", &command);
        let child = command
            .spawn()
            .with_context(|| "Could not spawn child process")?;
        eprintln!("spawned...");
        Ok(Self { child })
    }
}

impl Drop for CommandRunner {
    fn drop(&mut self) {
        eprintln!("dropping spawned background tasks");
        for background_task in self.tasks.iter_mut() {
            #[cfg(unix)]
            {
                // attempt to stop gracefully
                let pid = background_task.child.id();
                unsafe {
                    libc::kill(libc::pid_t::from_ne_bytes(pid.to_ne_bytes()), libc::SIGTERM);
                }

                for _ in 0..10 {
                    if background_task.child.try_wait().ok().flatten().is_some() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }

            if background_task.child.try_wait().ok().flatten().is_none() {
                // still alive? kill it with fire
                let _ = background_task.child.kill();
            }

            let _ = background_task.child.wait();
        }
    }
}
