use crate::command::Command;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "rover-fed2",
    about = "A utility for composing multiple subgraphs into a supergraph"
)]
pub struct RoverFed {
    #[structopt(subcommand)]
    command: Command,
}

impl RoverFed {
    #[cfg(feature = "composition-js")]
    pub fn run(&self) -> ! {
        let build_result = match &self.command {
            Command::Compose(command) => command.run(),
        };
        print!("{}", serde_json::json!(build_result));
        if build_result.is_ok() {
            std::process::exit(0)
        } else {
            std::process::exit(1);
        }
    }

    #[cfg(not(feature = "composition-js"))]
    pub fn run(&self) -> ! {
        let _ = match &self.command {
            Command::Compose(command) => command.run(),
        };
        std::process::exit(1)
    }
}
