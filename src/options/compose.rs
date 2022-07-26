use super::ProfileOpt;
use crate::command::install::license_accept;
use saucer::{clap, Parser};
use serde::Serialize;
#[derive(Debug, Clone, Serialize, Parser)]
pub struct PluginOpts {
    #[clap(flatten)]
    pub profile: ProfileOpt,

    /// Accept the elv2 license if you are using federation 2. Note that you only need to do this once per machine.
    #[clap(long = "elv2-license", parse(from_str = license_accept), case_insensitive = true, env = "APOLLO_ELV2_LICENSE")]
    pub elv2_license_accepted: Option<bool>,

    /// Skip the update check
    #[clap(long = "skip-update")]
    pub skip_update: bool,
}
