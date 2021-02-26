use crate::command::RoverStdout;
use crate::Result;
use crate::PKG_VERSION;
use serde::Serialize;
use std::env;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Info {}

impl Info {
    pub fn run(&self) -> Result<RoverStdout> {
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

        eprintln!(
            "Rover Info:\nVersion: {}\nInstall Location: {}\nOS: {}\nShell: {}",
            PKG_VERSION, location, os, shell
        );

        Ok(RoverStdout::None)
    }
}
