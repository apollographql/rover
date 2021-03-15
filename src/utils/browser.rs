use crate::{anyhow, Result};

use ansi_term::Colour::{Cyan, Yellow};
use std::process::Command;

pub fn open(url: &str) -> Result<()> {
    let yellow_browser_var = format!("{}", Yellow.normal().paint("$BROWSER"));
    let cyan_url = format!("{}", Cyan.normal().paint(url));
    
    if let Some(browser_override) = std::env::var_os("BROWSER") {
        eprintln!(
            "Opening {} with the application specified by {}.",
            &cyan_url, &yellow_browser_var
        );
        if let Err(e) = Command::new(&browser_override).arg(url).status() {
            Err(anyhow!(
                "Couldn't open browser {}: {}",
                browser_override.to_string_lossy(),
                e
            ))
        } else {
            Ok(())
        }
    } else {
        eprintln!("Opening {} with your default browser. This can be overridden by setting the {} environment variable.", &cyan_url, &yellow_browser_var);
        opener::open(url)?;
        Ok(())
    }?;

    Ok(())
}

