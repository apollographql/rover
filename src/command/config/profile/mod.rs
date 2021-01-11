mod auth;
mod delete;
mod list;
mod show;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;

use houston as config;

#[derive(Debug, Serialize, StructOpt)]  
/// Commands for managing config profiles
///
/// A profile is a saved set of global config options.
/// 
/// For more on how profiles work, see here: https://go.apollo.dev/rover-profiles
pub struct Profile {
    #[structopt(subcommand)]
    command: Command,
}

impl Profile {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        match &self.command {
            Command::Auth(command) => command.run(config),
            Command::List(command) => command.run(config),
            Command::Show(command) => command.run(config),
            Command::Delete(command) => command.run(config),
        }
    }
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// 🔑 Set a configuration profile's Apollo Studio API key
    Auth(auth::Auth),

    /// 👥 List all configuration profiles
    List(list::List),

    /// 👤 View a configuration profile's details
    Show(show::Show),

    /// 🗑  Delete a configuration profile
    Delete(delete::Delete),
}
