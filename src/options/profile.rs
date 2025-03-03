use std::fmt::Display;

use clap::Parser;
use serde::{Deserialize, Serialize};

pub const DEFAULT_PROFILE: &str = "default";

#[cfg_attr(test, derive(Default))]
#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct ProfileOpt {
    /// Name of configuration profile to use
    #[arg(long = "profile", default_value = DEFAULT_PROFILE)]
    #[serde(skip_serializing)]
    pub profile_name: String,
}

impl Display for ProfileOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.profile_name)
    }
}
