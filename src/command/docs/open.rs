use crate::{RoverOutput, RoverResult};

use super::shortlinks;

use anyhow::anyhow;
use clap::Parser;
use rover_std::{hyperlink, Spinner, Style};
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
        let painted_url = hyperlink(&url);

        if let Some(browser_override) = std::env::var_os("BROWSER") {
            let spinner = Spinner::new(&format!(
                "Opening {} with the application specified by {}.",
                &painted_url, &painted_browser_var
            ));

            if let Err(e) = Command::new(&browser_override).arg(&url).status() {
                spinner.stop();
                Err(anyhow!(
                    "Couldn't open docs with {}: {}",
                    browser_override.to_string_lossy(),
                    e
                ))
            } else {
                spinner.stop();
                Ok(())
            }
        } else {
            let spinner = Spinner::new(&format!(
                "Opening {} with your default browser. This can be overridden by setting the {} environment variable.",
                &painted_url, &painted_browser_var
            ));

            opener::open(&url)?;
            spinner.stop();
            Ok(())
        }?;

        Ok(RoverOutput::EmptySuccess)
    }
}
