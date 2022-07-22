use std::sync::mpsc::{Sender, SyncSender};

use ansi_term::Colour::{Cyan, Yellow};
use apollo_federation_types::build::SubgraphDefinition;
use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use dialoguer::Input;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use saucer::{anyhow, clap, Parser, Process, Utf8PathBuf};
use saucer::{Fs, Saucer};
use serde::Serialize;
use tempdir::TempDir;

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
        use std::sync::mpsc::{channel, sync_channel};

        let temp_dir = TempDir::new("subgraph")?;
        let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?.join("supergraph.yaml");
        let compose_saucer = SaucerFactory::get_compose_saucer(
            self.opts.compose_opts.clone(),
            override_install_path,
            client_config,
            temp_path.clone(),
        )?;

        compose_saucer.beam()?;
        let (router_sender, router_receiver) = sync_channel(1);
        let router_saucer = RouterSaucer::new(temp_path, router_sender);
        router_saucer.beam()?;
        let router_handle = match router_receiver.recv() {
            Ok(s) => Ok(s),
            Err(e) => Err(anyhow!("Could not start router {}", e)),
        };
        eprintln!("router is running...");

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
