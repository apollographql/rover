use clap::Parser;
use serde::Serialize;

use super::ProfileOpt;
use crate::options::LicenseAccepter;

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
    ///
    /// Set the `APOLLO_ROVER_SKIP_UPDATE` environment variable (to `1` or `true`)
    /// to disable all of Rover's auto-updating at once (this plugin check plus the
    /// rover self-update check, `--skip-update-check`).
    #[arg(long = "skip-update")]
    pub skip_update: bool,
}
