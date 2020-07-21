use anyhow::Result;
use rover::*;
use structopt::StructOpt;

fn main() -> Result<()> {
    logger::init();

    let cli = cli::Rover::from_args();
    log::debug!("Command structure {:?}", cli);
    cli.run()?;
    Ok(())
}
