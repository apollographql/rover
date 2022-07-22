use std::sync::mpsc::{sync_channel, Sender, SyncSender};

use ansi_term::Colour::{Cyan, Yellow};
use apollo_federation_types::build::SubgraphDefinition;
use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use dialoguer::Input;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use reqwest::blocking::Client;
use saucer::{anyhow, clap, Parser, Process, Utf8PathBuf};
use saucer::{Fs, ParallelSaucer, Saucer};
use serde::Serialize;
use tempdir::TempDir;

use crate::command::subgraph::introspect::Introspect;
use crate::command::supergraph::compose::Compose;
use crate::command::RoverOutput;
use crate::dot_apollo::{DotApollo, ProjectType};
use crate::options::{
    ComposeOpts, GraphRefOpt, OptionalGraphRefOpt, OptionalSchemaOpt, OptionalSubgraphOpt,
    ProfileOpt, SchemaOpt, SubgraphOpt,
};
use crate::utils::client::StudioClientConfig;
use crate::{error::RoverError, Result};

use rover_client::shared::GraphRef;

use netstat2::*;

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
    compose_opts: ComposeOpts,

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
        use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
        use netstat2::*;
        use std::{
            sync::mpsc::{channel, sync_channel},
            time::Duration,
        };
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
        // TODO: maybe use procs crate to try to detect the port
        eprintln!("{} is running...", &command);

        eprintln!("sleeping for 0.5 secs");
        std::thread::sleep(Duration::from_millis(500));

        let s = System::new_all();

        let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
        let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;
        let sockets_info = get_sockets_info(af_flags, proto_flags)?;

        let our_process = s.process(Pid::from_u32(command_handle.id())).unwrap();

        let mut maybe_endpoint = None;

        for (_, task_process) in our_process.tasks.iter() {
            for si in &sockets_info {
                if si.uid == *task_process.group_id().unwrap() {
                    if &si.local_addr().to_string() == "::" {
                        maybe_endpoint =
                            reqwest::Url::parse(&format!("http://0.0.0.0:{}", si.local_port()))
                                .ok();
                        break;
                    }
                }
            }
        }

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

        let temp_dir = TempDir::new("subgraph")?;
        let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?.join("supergraph.yaml");

        let compose_saucer = ComposeSaucer::new(
            self.opts.compose_opts.clone(),
            override_install_path,
            client_config,
            vec![SubgraphDefinition::new("my-subgraph", endpoint, sdl)],
            temp_path.clone(),
        );

        // let compose_saucer = SaucerFactory::get_compose_saucer(
        //     self.opts.compose_opts.clone(),
        //     override_install_path,
        //     client_config,
        //     temp_path.clone(),
        //     maybe_endpoint.clone(),
        // )?;

        compose_saucer.beam()?;
        let (router_sender, router_receiver) = sync_channel(1);
        let router_saucer = RouterSaucer::new(temp_path, router_sender);
        router_saucer.beam()?;
        let mut router_handle = match router_receiver.recv() {
            Ok(s) => Ok(s),
            Err(e) => Err(anyhow!("Could not start router {}", e)),
        }?;
        eprintln!("router is running...");
        rayon::join(
            || command_handle.wait().unwrap(),
            || router_handle.wait().unwrap(),
        );

        Ok(RoverOutput::EmptySuccess)
    }

    #[cfg(not(feature = "composition-js"))]
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        Err(RoverError::new(anyhow!(
            "rover dev is not supported on this platform"
        )))
    }
}

#[cfg(feature = "composition-js")]
#[derive(Debug, Clone)]
pub struct RouterSaucer {
    read_path: Utf8PathBuf,
    router_handle: SyncSender<std::process::Child>,
}

impl RouterSaucer {
    fn new(read_path: Utf8PathBuf, sender: SyncSender<std::process::Child>) -> Self {
        Self {
            read_path,
            router_handle: sender,
        }
    }
}

impl Saucer for RouterSaucer {
    fn description(&self) -> String {
        "router".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        eprintln!("starting router");
        let router_handle = std::process::Command::new("./.apollo/router")
            .args(&["--supergraph", self.read_path.as_str(), "--hot-reload"])
            .spawn()?;
        self.router_handle.send(router_handle)?;
        // Process::new(
        //     "./.apollo/router",
        //     &["--supergraph", self.read_path.as_str(), "--hot-reload"],
        // )
        // .run("")
        Ok(())
    }
}

#[cfg(feature = "composition-js")]
#[derive(Debug, Clone)]
pub struct ComposeSaucer {
    compose: Compose,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    supergraph_config: SupergraphConfig,
    write_path: Utf8PathBuf,
}

impl ComposeSaucer {
    fn new(
        compose_opts: ComposeOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        subgraph_definitions: Vec<SubgraphDefinition>,
        write_path: Utf8PathBuf,
    ) -> Self {
        let mut supergraph_config = SupergraphConfig::from(subgraph_definitions);
        supergraph_config.set_federation_version(FederationVersion::LatestFedTwo);
        Self {
            compose: Compose::new(compose_opts),
            override_install_path,
            client_config,
            supergraph_config,
            write_path,
        }
    }
}

impl Saucer for ComposeSaucer {
    fn description(&self) -> String {
        "composition".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        match self.compose.compose(
            self.override_install_path.clone(),
            self.client_config.clone(),
            &mut self.supergraph_config.clone(),
        ) {
            Ok(build_result) => match &build_result {
                RoverOutput::CompositionResult {
                    supergraph_sdl,
                    hints,
                    federation_version,
                } => {
                    // let _ = build_result.print();
                    Fs::write_file(&self.write_path, supergraph_sdl, "")?;
                    Ok(())
                }
                _ => unreachable!(),
            },
            Err(e) => Err(anyhow!("{}", e)),
        }
    }
}

pub struct SaucerFactory {}

impl SaucerFactory {
    fn get_compose_saucer(
        compose_opts: ComposeOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        write_path: Utf8PathBuf,
    ) -> Result<ComposeSaucer> {
        let maybe_multi_config = DotApollo::subgraph_from_yaml()?;
        if let Some(multi_config) = maybe_multi_config {
            let subgraphs =
                multi_config.get_all_subgraphs(true, &client_config, &compose_opts.profile)?;
            Ok(ComposeSaucer::new(
                compose_opts.clone(),
                override_install_path,
                client_config,
                subgraphs,
                write_path,
            ))
        } else {
            Err(RoverError::new(anyhow!("found no valid subgraph definitions in .apollo/config.yaml. please run `rover subgraph init` or run this command in a project with a .apollo directory")))
        }
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
                self.sdl_sender.send(Ok(s))?;
            }
            (Err(_), Ok(s)) => {
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
