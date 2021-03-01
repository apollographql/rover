use ansi_term::Colour::Cyan;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::{Result, PKG_VERSION};

use semver::Version;

use rover_client::releases::get_latest_release;

#[derive(Debug, Serialize, StructOpt)]
pub struct Check {
    // TODO: support prerelease check through flag
}

impl Check {
    pub fn run(&self) -> Result<RoverStdout> {
        let latest = get_latest_release()?;
        let update_available = needs_update(&latest, PKG_VERSION)?;

        if update_available {
            eprintln!(
          "There is a newer version of Rover available for download: {} (currently running v{})\n\nFor instructions on how to install the latest version of Rover, see {}", 
          Cyan.normal().paint(format!("v{}", latest)), 
          PKG_VERSION,
          Cyan.normal().paint("https://go.apollo.dev/r/start")
        );
        } else {
            eprintln!("Rover is up to date!");
        }

        Ok(RoverStdout::None)
    }
}

fn needs_update(latest: &str, running: &str) -> Result<bool> {
    let latest = Version::parse(latest)?;
    let running = Version::parse(running)?;
    Ok(latest > running)
}
