use std::fmt::Display;

use serde::{Deserialize, Serialize};

pub const DEFAULT_PROFILE: &str = "default";

/// The profile a command resolved to. Built once, from `Rover`'s global
/// `--profile` flag, and passed down to whichever command needs it - this is
/// no longer a `clap` arg in its own right (see `Rover::profile_name`).
#[cfg_attr(test, derive(Default))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileOpt {
    #[serde(skip_serializing)]
    pub profile_name: String,
}

impl Display for ProfileOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.profile_name)
    }
}
