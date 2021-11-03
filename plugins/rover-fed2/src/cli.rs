use structopt::StructOpt;

use apollo_federation_types::BuildErrors;

use crate::command::Command;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "rover-fed2",
    about = "A utility for composing multiple subgraphs into a supergraph"
)]
pub struct RoverFed {
    #[structopt(subcommand)]
    command: Command,

    /// Print output as JSON.
    #[structopt(long, global = true)]
    json: bool,
}

impl RoverFed {
    pub fn run(&self) -> ! {
        let composition_result = match &self.command {
            Command::Compose(command) => command.run(),
        };
        match composition_result {
            Ok(composition_output) => {
                if self.json {
                    print!("{}", serde_json::json!(composition_output));
                } else {
                    for hint in composition_output.hints {
                        eprintln!("WARN: {}", hint);
                    }
                    println!("{}", composition_output.supergraph_sdl)
                }
                std::process::exit(0);
            }
            Err(composition_err) => {
                if self.json {
                    if let Some(build_errors) = composition_err.downcast_ref::<BuildErrors>() {
                        print!("{}", serde_json::json!(build_errors));
                    } else {
                        println!(
                            "{}",
                            serde_json::json!({"message": composition_err.to_string()})
                        )
                    }
                } else {
                    println!("{}", composition_err);
                }
                std::process::exit(1);
            }
        }
    }
}
