use crate::{anyhow, command::RoverOutput, Result};

use super::shortlinks;

use ansi_term::Colour::{Cyan, Yellow};
use clap::Parser;
use serde::Serialize;

use std::process::Command;

#[derive(Debug, Serialize, Parser)]
pub struct Open {
    #[clap(name = "slug", default_value = "docs", possible_values = shortlinks::possible_shortlinks())]
    slug: String,
}

impl Open {
    pub fn run(&self) -> Result<RoverOutput> {
        let url = shortlinks::get_url_from_slug(&self.slug);
        let yellow_browser_var = format!("{}", Yellow.normal().paint("$BROWSER"));
        let cyan_url = format!("{}", Cyan.normal().paint(&url));

        if let Some(browser_override) = std::env::var_os("BROWSER") {
            eprintln!(
                "Opening {} with the application specified by {}.",
                &cyan_url, &yellow_browser_var
            );
            if let Err(e) = Command::new(&browser_override).arg(&url).status() {
                Err(anyhow!(
                    "Couldn't open docs with {}: {}",
                    browser_override.to_string_lossy(),
                    e
                ))
            } else {
                Ok(())
            }
        } else {
            eprintln!("Opening {} with your default browser. This can be overridden by setting the {} environment variable.", &cyan_url, &yellow_browser_var);
            opener::open(&url)?;
            Ok(())
        }?;

        Ok(RoverOutput::EmptySuccess)
    }
}
