use std::path::{Path, PathBuf};

use anyhow::anyhow;
use apollo_federation_types::config::{FederationVersion, SchemaSource};
use camino::Utf8PathBuf;
use clap::Parser;
use rover_client::shared::GraphRef;
use semver::Version;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    command::connector::{
        analyze::AnalyzeCurl, generate::GenerateConnector, list::ListConnector, run::RunConnector,
        test::TestConnector,
    },
    composition::{
        get_supergraph_binary,
        pipeline::{CompositionPipeline, state::Run},
        supergraph::config::lazy::LazilyResolvedSubgraph,
    },
    options::PluginOpts,
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
};

pub mod analyze;
pub mod generate;
pub mod list;
pub mod run;
pub mod test;

#[derive(Debug, Serialize, Parser)]
pub struct Connector {
    #[clap(subcommand)]
    command: Command,

    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    /// The version of Apollo Federation to use for composition. If no version is supplied, Rover
    /// will use the latest 2.x version
    #[arg(long = "federation-version")]
    pub federation_version: Option<FederationVersion>,

    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[serde(skip_serializing)]
    #[arg(long = "supergraph-config")]
    pub supergraph_yaml: Option<FileDescriptorType>,

    /// A [`GraphRef`] that is accessible in Apollo Studio.
    /// This is used to initialize your supergraph with the values contained in this variant.
    ///
    /// This is analogous to providing a supergraph.yaml file with references to your graph variant in studio.
    ///
    /// If used in conjunction with `--config`, the values presented in the supergraph.yaml will take precedence over these values.
    #[arg(long = "graph-ref")]
    pub graph_ref: Option<GraphRef>,
}

#[derive(Debug, Parser, Serialize)]
#[clap(about = "Work with Apollo Connectors")]
pub enum Command {
    /// Generate a schema with connectors from a collection of analyzed data
    Generate(GenerateConnector),
    /// Analyze one or more requests for use in generating
    /// a Connector
    Analyze(AnalyzeCurl),
    /// Run a single connector
    Run(RunConnector),
    /// Run tests for one or more connectors
    Test(TestConnector),
    /// List all available connectors
    List(ListConnector),
}

impl Connector {
    pub(crate) async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        use Command::*;

        let supergraph_yaml = if let Some(supergraph_yaml) = &self.supergraph_yaml {
            Some(supergraph_yaml.clone())
        } else if Path::new("supergraph.yaml").exists() {
            Some(FileDescriptorType::File("supergraph.yaml".into()))
        } else {
            None
        };

        let composition_pipeline = get_supergraph_binary(
            self.federation_version.clone(),
            client_config,
            override_install_path,
            self.plugin_opts.clone(),
            supergraph_yaml.clone(),
            self.graph_ref.clone(),
        )
        .await?;
        let default_subgraph = default_subgraph(&supergraph_yaml, &composition_pipeline).await;
        let supergraph_binary = composition_pipeline.state.supergraph_binary?;
        let minimum_version = Version::parse("2.12.0-preview.9")?;
        let current_version = supergraph_binary.version();
        if current_version < &minimum_version {
            return Err(anyhow!(
                "You must use federation {minimum_version} or greater with `rover connectors` commands. \
                Your current version is {current_version}. \
                Update your `supergraph.yaml` or use --federation-version to specify a compatible version.",
            ).into());
        }

        match &self.command {
            Generate(command) => command.run(supergraph_binary).await,
            Test(command) => command.run(supergraph_binary, default_subgraph).await,
            Run(command) => command.run(supergraph_binary, default_subgraph).await,
            List(command) => command.run(supergraph_binary, default_subgraph).await,
            Analyze(command) => command.run(supergraph_binary).await,
        }
    }
}

async fn default_subgraph(
    supergraph_yaml: &Option<FileDescriptorType>,
    composition_pipeline: &CompositionPipeline<Run>,
) -> Option<PathBuf> {
    let Some(FileDescriptorType::File(supergraph_yaml_path)) = supergraph_yaml else {
        return None;
    };
    let (supergraph, _) = composition_pipeline
        .state
        .resolver
        .lazily_resolve_subgraphs(&supergraph_yaml_path.parent()?.to_path_buf())
        .await
        .ok()?;
    if supergraph.subgraphs().len() == 1
        && let Some(SchemaSource::File { file }) = supergraph
            .subgraphs()
            .values()
            .next()
            .map(LazilyResolvedSubgraph::schema)
    {
        Some(supergraph_yaml_path.as_std_path().join(file))
    } else {
        None
    }
}
