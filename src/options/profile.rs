use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct ProfileOpt {
    /// Name of configuration profile to use
    #[clap(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    pub profile_name: String,
}
