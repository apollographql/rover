use std::io::{self, prelude::*, BufReader};
use std::process::Stdio;
use std::sync::mpsc::{sync_channel, SyncSender};

use apollo_federation_types::build::SubgraphDefinition;
use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use dialoguer::Input;
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
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
use crate::Result;

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
    local_url: Option<String>,
}

impl Dev {
    #[cfg(feature = "composition-js")]
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        use dialoguer::Select;
        use netstat2::*;
        use std::time::Duration;
        use sysinfo::{Pid, PidExt, ProcessExt, System, SystemExt};
        let command: String = Input::new()
            .with_prompt("what command do you use to start your graph?")
            .interact_text()?;

        let (command_sender, command_receiver) = sync_channel(2);
        let command_saucer = CommandSaucer::new(command.to_string(), command_sender);
        command_saucer.beam()?;
        let mut command_handle = match command_receiver.recv() {
            Ok(s) => Ok(s),
            Err(e) => Err(anyhow!("Could not start `{}` {}", &command, e)),
        }?;

        let command_pid = command_handle.id();

        let command_join_handle = std::thread::spawn(move || command_handle.wait());

        eprintln!("sleeping for 0.5 secs");
        std::thread::sleep(Duration::from_millis(500));

        let s = System::new_all();

        let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
        let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;
        let sockets_info = get_sockets_info(af_flags, proto_flags)?;

        let mut possible_endpoints = Vec::new();
        if let Some(command_process) = s.process(Pid::from_u32(command_pid)) {
            eprintln!("{} is running...", &command);
            for (_, task_process) in command_process.tasks.iter() {
                for si in &sockets_info {
                    if si.uid == *task_process.group_id().unwrap() {
                        if &si.local_addr().to_string() == "::" {
                            if let Ok(possible_endpoint) =
                                reqwest::Url::parse(&format!("http://0.0.0.0:{}", si.local_port()))
                            {
                                possible_endpoints.push(possible_endpoint);
                            }
                        }
                    }
                }
            }
        } else {
            return Err(anyhow!("`{}` failed to start.", &command).into());
        }

        let maybe_endpoint = match possible_endpoints.len() {
            0 => None,
            1 => Some(possible_endpoints[0].clone()),
            _ => {
                if let Ok(endpoint_index) = Select::new()
                    .items(&possible_endpoints)
                    .default(0)
                    .interact()
                {
                    Some(possible_endpoints[endpoint_index].clone())
                } else {
                    None
                }
            }
        };

        let endpoint = if let Some(endpoint) = maybe_endpoint {
            eprintln!("detected endpoint {}", &endpoint);
            endpoint
        } else {
            let endpoint: String = Input::new()
                .with_prompt("what endpoint is your graph running on?")
                .interact_text()?;
            let endpoint = reqwest::Url::parse(&endpoint)?;
            endpoint
        };

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
        let socket_addr = "/tmp/supergraph.sock";
        if let Ok(mut subgraph_stream) = LocalSocketStream::connect(socket_addr) {
            eprintln!(
                "a `rover dev` sesssion is already running on this computer, extending it..."
            );
            subgraph_stream
                .write(this_subgraph_json.as_bytes())
                .context("could not inform other `rover dev` session about your subgraph")?;
        } else {
            eprintln!("no `rover dev` session is running, starting a supergraph from scratch...");
            let _ = std::fs::remove_file(socket_addr);
            let mut compose_saucer = ComposeSaucer::new(
                self.opts.plugin_opts.clone(),
                override_install_path.clone(),
                client_config.clone(),
                vec![this_subgraph],
                temp_path.clone(),
            );

            compose_saucer.beam()?;

            let subgraph_listener = LocalSocketListener::bind(socket_addr).with_context(|| {
                format!("could not start local socket server at {}", socket_addr)
            })?;

            let (router_sender, router_receiver) = sync_channel(1);
            let router_saucer = RouterSaucer::new(
                temp_path,
                router_sender,
                self.opts.plugin_opts.clone(),
                override_install_path,
                client_config,
            );
            router_saucer.beam()?;
            let mut router_handle = match router_receiver.recv() {
                Ok(s) => Ok(s),
                Err(e) => Err(anyhow!("Could not start router {}", e)),
            }?;
            // TODO: replace this with something that polls a health check on the router
            std::thread::sleep(Duration::from_millis(500));
            eprintln!("router is running! head to http://localhost:4000 to query your supergraph");
            for incoming_connection in subgraph_listener.incoming().filter_map(handle_socket_error)
            {
                let mut connection_reader = BufReader::new(incoming_connection);
                let mut subgraph_definition_buffer = String::new();
                if connection_reader
                    .read_line(&mut subgraph_definition_buffer)
                    .is_ok()
                {
                    let subgraph_definition: SubgraphDefinition =
                        serde_json::from_str(&subgraph_definition_buffer)
                            .context("could not read incoming subgraph info")?;
                    compose_saucer.add_subgraph(subgraph_definition)?;
                } else {
                    eprintln!("could not read incoming line from socket stream");
                }
            }
            let _ = router_handle.wait();
        }

        let _ = command_join_handle.join();

        Ok(RoverOutput::EmptySuccess)
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
pub struct RouterSaucer {
    read_path: Utf8PathBuf,
    router_handle: SyncSender<std::process::Child>,
    opts: PluginOpts,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
}

impl RouterSaucer {
    fn new(
        read_path: Utf8PathBuf,
        sender: SyncSender<std::process::Child>,
        opts: PluginOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Self {
        Self {
            read_path,
            router_handle: sender,
            opts,
            override_install_path,
            client_config,
        }
    }
}

impl Saucer for RouterSaucer {
    fn description(&self) -> String {
        "router".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        let plugin = Plugin::Router;
        let plugin_name = plugin.get_name();
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

        eprintln!("starting router, watching {}", &self.read_path);
        let router_handle = std::process::Command::new("./.apollo/router")
            .args(&["--supergraph", self.read_path.as_str(), "--hot-reload"])
            // .stdout(Stdio::null())
            // .stderr(Stdio::null())
            .spawn()?;
        self.router_handle.send(router_handle)?;
        Ok(())
    }
}

#[cfg(feature = "composition-js")]
#[derive(Debug, Clone)]
pub struct ComposeSaucer {
    compose: Compose,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    subgraph_definitions: Vec<SubgraphDefinition>,
    write_path: Utf8PathBuf,
}

impl ComposeSaucer {
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

    fn add_subgraph(&mut self, subgraph_definition: SubgraphDefinition) -> saucer::Result<()> {
        self.subgraph_definitions.push(subgraph_definition);
        self.beam()
    }
}

impl Saucer for ComposeSaucer {
    fn description(&self) -> String {
        "composition".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
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
                    // let _ = build_result.print();
                    let _ = std::fs::remove_file(&self.write_path);
                    Fs::write_file(&self.write_path, supergraph_sdl, "")?;
                    eprintln!("wrote updated supergraph schema to {}", &self.write_path);
                    Ok(())
                }
                _ => unreachable!(),
            },
            Err(e) => Err(anyhow!("{}", e)),
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

#[derive(Clone, Debug)]
pub(crate) struct CommandSaucer {
    command: String,
    command_handle: SyncSender<std::process::Child>,
}

impl CommandSaucer {
    pub(crate) fn new(command: String, command_handle: SyncSender<std::process::Child>) -> Self {
        Self {
            command,
            command_handle,
        }
    }
}

impl Saucer for CommandSaucer {
    fn beam(&self) -> saucer::Result<()> {
        let args: Vec<&str> = self.command.split(" ").collect();
        let (bin, args) = match args.len() {
            0 => Err(anyhow!("the command you passed is empty")),
            1 => Ok((args[0], Vec::new())),
            _ => Ok((args[0], Vec::from_iter(args[1..].iter()))),
        }?;
        eprintln!("starting `{}`", &self.command);
        let command_handle = std::process::Command::new(bin).args(args).spawn()?;
        self.command_handle.send(command_handle)?;
        Ok(())
    }

    fn description(&self) -> String {
        "graph runner".to_string()
    }
}
