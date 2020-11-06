use atty::{self, Stream};

/// RoverStdout defines all of the different types of data that are printed
/// to `stdout`. Every one of Rover's commands should return `anyhow::Result<RoverStdout>`
/// If the command needs to output some type of data, it should be structured
/// in this enum, and its print logic should be handled in `RoverStdout::print`
///
/// Not all commands will output machine readable information, and those should
/// return `Ok(RoverStdout::None)`. If a new command is added and it needs to
/// return something that is not described well in this enum, it should be added.
#[derive(Clone, PartialEq, Debug)]
pub enum RoverStdout {
    SDL(String),
    SchemaHash(String),
    None,
}

impl RoverStdout {
    pub fn print(&self) {
        match self {
            RoverStdout::SDL(sdl) => {
                // we check to see if stdout is redirected
                // if it is, we don't print the content descriptor
                // this is because it would look strange to see
                // SDL:
                // and nothing after the colon if you piped the output
                // to another process or a file.
                if atty::is(Stream::Stdout) {
                    tracing::info!("SDL:");
                }
                println!("{}", &sdl);
            }
            RoverStdout::SchemaHash(hash) => {
                if atty::is(Stream::Stdout) {
                    tracing::info!("Schema Hash:");
                }
                println!("{}", &hash);
            }
            RoverStdout::None => (),
        }
    }
}
