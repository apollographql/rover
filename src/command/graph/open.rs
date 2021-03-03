use crate::{anyhow, command::RoverStdout, Result};

use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::config::frontend;

use std::process::Command;

#[derive(Debug, Serialize, StructOpt)]
pub struct Open {}

impl Open {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let client = client_config.get_client(&self.profile_name)?;

        let base_url = frontend::run(client);
        // let url = shortlinks::get_url_from_slug(&self.slug);
        // let yellow_browser_var = format!("{}", Yellow.normal().paint("$BROWSER"));
        // let cyan_url = format!("{}", Cyan.normal().paint(&url));

        // if let Some(browser_override) = std::env::var_os("BROWSER") {
        //     eprintln!(
        //         "Opening {} with the application specified by {}.",
        //         &cyan_url, &yellow_browser_var
        //     );
        //     if let Err(e) = Command::new(&browser_override).arg(&url).status() {
        //         Err(anyhow!(
        //             "Couldn't open docs with {}: {}",
        //             browser_override.to_string_lossy(),
        //             e
        //         ))
        //     } else {
        //         Ok(())
        //     }
        // } else {
        //     eprintln!("Opening {} with your default browser. This can be overridden by setting the {} environment variable.", &cyan_url, &yellow_browser_var);
        //     opener::open(&url)?;
        //     Ok(())
        // }?;

        Ok(RoverStdout::None)
    }
}
