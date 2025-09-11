use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use clap::Parser;
use derive_getters::Getters;
use serde::Serialize;

use crate::command::connector::{
    analyze::AnalyzeCurl, generate::GenerateConnector, list::ListConnector, run::RunConnector,
    test::TestConnector,
};
use crate::composition::pipeline::CompositionPipelineError;
use crate::composition::supergraph::binary::SupergraphBinary;
use crate::composition::supergraph::install::InstallSupergraph;
use crate::options::{LicenseAccepter, PluginOpts};
use crate::utils::client::StudioClientConfig;
use crate::utils::effect::install::InstallBinary;
use crate::{RoverOutput, RoverResult};

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
    opts: ConnectorOpts,
}

#[derive(Clone, Debug, Serialize, Parser, Getters)]
pub struct ConnectorOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    /// The version of Apollo Federation to use for composition. If no version is supplied, Rover
    /// will use the latest 2.x version
    #[arg(long = "federation-version")]
    pub federation_version: Option<FederationVersion>,
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

        // TODO: This will probably need to change st some point but it's the best we can do for now
        // In the future we may want to consider the environment variables, the supergraph.yaml, or the version in the schema itself
        let federation_version = self
            .opts
            .clone()
            .federation_version
            .unwrap_or(FederationVersion::LatestFedTwo);

        let supergraph_binary = install_supergraph_binary(
            federation_version,
            client_config,
            override_install_path,
            self.opts.plugin_opts.elv2_license_accepter,
            self.opts.plugin_opts.skip_update,
        )
        .await?;

        match &self.command {
            Generate(command) => command.run(supergraph_binary).await,
            Test(command) => command.run(supergraph_binary).await,
            Run(command) => command.run(supergraph_binary).await,
            List(command) => command.run(supergraph_binary).await,
            Analyze(command) => command.run(supergraph_binary).await,
        }
    }
}

async fn install_supergraph_binary(
    federation_version: FederationVersion,
    studio_client_config: StudioClientConfig,
    override_install_path: Option<Utf8PathBuf>,
    elv2_license_accepter: LicenseAccepter,
    skip_update: bool,
) -> Result<SupergraphBinary, CompositionPipelineError> {
    let binary = InstallSupergraph::new(federation_version, studio_client_config)
        .install(override_install_path, elv2_license_accepter, skip_update)
        .await?;

    Ok(binary)
}
