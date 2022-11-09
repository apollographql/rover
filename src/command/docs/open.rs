use crate::{RoverOutput, RoverResult};

use super::shortlinks;

use anyhow::anyhow;
use clap::Parser;
use rover_std::Style;
use serde::Serialize;

use std::process::Command;

#[derive(Debug, Serialize, Parser)]
pub struct Open {
    #[arg(value_name = "SLUG", default_value = "docs", value_parser = shortlinks::possible_shortlinks())]
    slug: String,
}

impl Open {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let url = shortlinks::get_url_from_slug(&self.slug);
        let painted_browser_var = Style::Command.paint("$BROWSER");
        let painted_url = Style::Link.paint(&url);

        if let Some(browser_override) = std::env::var_os("BROWSER") {
            eprintln!(
                "Opening {} with the application specified by {}.",
                &painted_url, &painted_browser_var
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
            eprintln!("Opening {} with your default browser. This can be overridden by setting the {} environment variable.", &painted_url, &painted_browser_var);
            opener::open(&url)?;
            Ok(())
        }?;

        Ok(RoverOutput::EmptySuccess)
    }
}
