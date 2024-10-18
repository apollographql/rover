use super::ProfileOpt;
use crate::options::LicenseAccepter;

use clap::Parser;
use serde::Serialize;

#[cfg(all(feature = "composition-js", not(feature = "dev-next")))]
use crate::{utils::client::StudioClientConfig, RoverResult};

#[cfg_attr(test, derive(Default))]
#[derive(Debug, Clone, Serialize, Parser)]
pub struct PluginOpts {
    #[clap(flatten)]
    pub profile: ProfileOpt,

    #[clap(flatten)]
    pub elv2_license_accepter: LicenseAccepter,

    /// Skip the update check for a plugin.
    ///
    /// Passing this flag will attempt to use the latest compatible version of a plugin already installed on this machine.
    #[arg(long = "skip-update")]
    pub skip_update: bool,
}

#[cfg(all(feature = "composition-js", not(feature = "dev-next")))]
impl PluginOpts {
    pub fn prompt_for_license_accept(&self, client_config: &StudioClientConfig) -> RoverResult<()> {
        self.elv2_license_accepter
            .require_elv2_license(client_config)
    }
}
