mod cli;

pub(crate) mod command;

use cli::RoverFed;

use structopt::StructOpt;

fn main() -> ! {
    let app = RoverFed::from_args();
    app.run();
}
