use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use crate::utils::client::StudioClientConfig;
use crate::{anyhow, command::RoverStdout, Result};

use std::path::Path;

#[derive(Debug, Serialize, StructOpt)]
pub struct Compose {
    /// The relative path to the supergraph configuration file.
    #[structopt(long = "config")]
    #[serde(skip_serializing)]
    config_path: Option<Utf8PathBuf>,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Compose {
    pub fn run(&self, _client_config: StudioClientConfig) -> Result<RoverStdout> {
        if Path::new("/lib/x86_64-linux-gnu/libc.so.6").exists() {
            Err(anyhow!("This version of Rover was built with `musl` and does not support this command. It looks like you have `glibc` installed, so if you install a Rover binary built for `gnu` then this command will work.").into())
        } else {
            Err(anyhow!("This version of Rover was built with `musl` and does not support this command. You will need a system with kernel 2.6.32+ and glibc 2.11+ ").into())
        }
    }
}
