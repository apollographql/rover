use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use houston as config;

use crate::command::RoverStdout;
#[derive(Debug, Serialize, StructOpt)]
/// View a configuration profile's details
/// 
/// If a profile has sensitive info, like an API key, pass --sensitive to see it.
pub struct Show {
    #[structopt(default_value = "default")]
    #[serde(skip_serializing)]
    name: String,

    #[structopt(long = "sensitive")]
    sensitive: bool,
}

impl Show {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        let opts = config::LoadOpts {
            sensitive: self.sensitive,
        };

        let profile = config::Profile::load(&self.name, &config, opts).map_err(|e| {
            let context = match e {
            config::HoustonProblem::NoNonSensitiveConfigFound(_) => {
                "Could not show any profile information. Try re-running with the `--sensitive` flag"
            }
            _ => "Could not load profile",
        };
            anyhow::anyhow!(e).context(context)
        })?;

        tracing::info!("{}: {}", &self.name, profile);
        Ok(RoverStdout::None)
    }
}
