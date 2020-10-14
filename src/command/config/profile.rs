use anyhow::Result;
use houston as config;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Profile {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// 🎅 List all of your configuration profiles
    List,
    /// 👀 See a specific profile's values
    Show(Show),
    /// 🪓 Delete a specific profile
    Delete(Delete),
}

#[derive(Debug, StructOpt)]
pub struct Show {
    #[structopt(default_value = "default")]
    name: String,
    #[structopt(long = "sensitive")]
    sensitive: bool,
}

#[derive(Debug, StructOpt)]
pub struct Delete {
    name: String,
}

impl Profile {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::List => {
                let profiles = config::Profile::list()?;
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
                let profile = config::Profile::load(&s.name, opts)?;
                tracing::info!("{}: {}", &s.name, profile);
                Ok(())
            }
            Command::Delete(d) => {
                config::Profile::delete(&d.name)?;
                tracing::info!("Successfully deleted profile {}", &d.name);
                Ok(())
            }
        }
    }
}
