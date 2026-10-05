use clap::Parser;
use serde::Serialize;

use crate::options::LicenseAccepter;

#[cfg_attr(test, derive(Default))]
#[derive(Debug, Clone, Serialize, Parser)]
pub struct PluginOpts {
    #[clap(flatten)]
    pub elv2_license_accepter: LicenseAccepter,

    /// Never download a plugin: use only what is installed.
    ///
    /// Passing this flag uses the latest compatible version of a plugin already installed on this machine,
    /// and fails, naming the plugin, if none is. It forbids downloading even when the
    /// `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` setting permits it.
    ///
    /// Without an opt-in, Rover already uses only installed plugins, and a plugin that isn't installed
    /// stops the command; install it first with `rover plugin install`. This flag also skips checking
    /// the plugin registry for a newer release when Rover has opted in.
    ///
    /// Set the `APOLLO_ROVER_SKIP_UPDATE` environment variable (to `1` or `true`)
    /// to disable all of Rover's auto-updating at once (this plugin check plus the
    /// rover self-update check, `--skip-update-check`).
    #[arg(long = "skip-update")]
    pub skip_update: bool,
}
