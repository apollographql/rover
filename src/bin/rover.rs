use anyhow::Result;
use rover::*;
use structopt::StructOpt;

fn main() -> Result<()> {
    logger::init();

    let cli = cli::Rover::from_args();
    // WHY IS THIS LINE LOAD BEARING?????
    // log::debug!("Command structure {:?}", cli);
    cli.run()?;
    Ok(())
}
