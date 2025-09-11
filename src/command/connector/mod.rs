use crate::command::connector::{
    analyze::AnalyzeCurl, generate::GenerateConnector, list::ListConnector, run::RunConnector,
    test::TestConnector,
};
use crate::composition::get_supergraph_binary;
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::FileDescriptorType;
use crate::{RoverOutput, RoverResult};
use anyhow::anyhow;
use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use clap::Parser;
use rover_client::shared::GraphRef;
use semver::Version;
use serde::Serialize;
use std::path::Path;

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
    // Run tests for one or more connectors
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
            supergraph_yaml,
            self.graph_ref.clone(),
        )
        .await?;
        let supergraph_binary = composition_pipeline.state.supergraph_binary?;
        let minimum_version = Version::parse("2.12.0-preview.7")?;
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
            Test(command) => command.run(supergraph_binary).await,
            Run(command) => command.run(supergraph_binary).await,
            List(command) => command.run(supergraph_binary).await,
            Analyze(command) => command.run(supergraph_binary).await,
        }
    }
}
