use crate::{PKG_VERSION, RoverOutput, RoverResult};

use calm_io::stderrln;
use clap::Parser;
use serde::Serialize;
use std::env;

#[derive(Debug, Serialize, Parser)]
pub struct Info {}

impl Info {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let os = os_info::get();

        // something like "/usr/bin/zsh" or "Unknown"
        let shell = env::var("SHELL").unwrap_or_else(|_| "Unknown".to_string());

        let location = match env::current_exe() {
            Ok(path) => path
                .into_os_string()
                .into_string()
                .unwrap_or_else(|_| "Unknown".to_string()),
            Err(_) => "Unknown".to_string(),
        };

        stderrln!(
            "Rover Info:\nVersion: {}\nInstall Location: {}\nOS: {}\nShell: {}",
            PKG_VERSION,
            location,
            os,
            shell
        )?;

        Ok(RoverOutput::EmptySuccess)
    }
}
