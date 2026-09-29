mod output;

use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

use self::output::PluginInstallOutput;
use crate::{
    PKG_NAME, RoverOutput, RoverResult,
    command::install::{Plugin, PluginInstaller, installer},
    options::LicenseAccepter,
    plugin::error::RequestOrigin,
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct PluginInstall {
    /// The plugin to install, as `<NAME>@<VERSION>`, e.g. `supergraph@=2.9.3` or `router@2`
    #[arg(value_name = "NAME@VERSION")]
    pub(crate) plugin: Plugin,

    /// Overwrite any existing binary without prompting for confirmation.
    #[arg(long = "force", short = 'f')]
    pub(crate) force: bool,

    #[clap(flatten)]
    pub(crate) elv2_license_accepter: LicenseAccepter,
}

impl PluginInstall {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        if self.plugin.requires_elv2_license() {
            self.elv2_license_accepter
                .require_elv2_license(&client_config)?;
        }
        let rover_installer = installer(PKG_NAME, self.force, override_install_path)?;
        let installed = PluginInstaller::new(client_config, rover_installer, self.force)
            .requested_by(Some(RequestOrigin::PluginArgument))
            .install(&self.plugin, false)
            .await?;

        Ok(RoverOutput::CliOutput(Box::new(PluginInstallOutput {
            plugins: vec![installed],
        })))
    }
}
