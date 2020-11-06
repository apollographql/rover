use anyhow::{Context, Result};
use houston as config;
use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Profile {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// 🎅 List all of your configuration profiles
    List,

    /// 👀 See a specific profile's values
    Show(Show),

    /// 🪓 Delete a specific profile
    Delete(Delete),
}

#[derive(Debug, Serialize, StructOpt)]
pub struct Show {
    #[structopt(default_value = "default")]
    #[serde(skip_serializing)]
    name: String,

    #[structopt(long = "sensitive")]
    sensitive: bool,
}

#[derive(Debug, Serialize, StructOpt)]
pub struct Delete {
    #[serde(skip_serializing)]
    name: String,
}

impl Profile {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::List => {
                let profiles = config::Profile::list().context("Could not list profiles.")?;
                if profiles.is_empty() {
                    tracing::info!("No profiles found.")
                } else {
                    tracing::info!("Profiles:");
                    for profile in profiles {
                        tracing::info!("{}", profile);
                    }
                }
                Ok(())
            }
            Command::Show(s) => {
                let opts = config::LoadOpts {
                    sensitive: s.sensitive,
                };

                let profile = config::Profile::load(&s.name, opts).map_err(|e| {
                    let context = match e {
                        config::HoustonProblem::NoNonSensitiveConfigFound(_) => {
                            "Could not show any profile information. Try re-running with the `--sensitive` flag"
                        }
                        _ => "Could not load profile",
                    };
                    anyhow::anyhow!(e).context(context)
                })?;

                tracing::info!("{}: {}", &s.name, profile);
                Ok(())
            }
            Command::Delete(d) => {
                config::Profile::delete(&d.name).context("Could not delete profile.")?;
                tracing::info!("Successfully deleted profile \"{}\"", &d.name);
                Ok(())
            }
        }
    }
}
