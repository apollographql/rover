mod auth;
mod delete;
mod list;
mod show;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Profile {
    #[structopt(subcommand)]
    command: Command,
}

impl Profile {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::Auth(command) => command.run(),
            Command::List(command) => command.run(),
            Command::Show(command) => command.run(),
            Command::Delete(command) => command.run(),
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
