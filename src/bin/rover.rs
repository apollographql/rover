use anyhow::Result;
use rover::*;
use structopt::StructOpt;

fn main() -> Result<()> {
    let cli = cli::Rover::from_args();
    timber::init(cli.log_level);
    tracing::trace!(command_structure = ?cli);
    println!("{:?}", &cli);
    cli.run()?;
    Ok(())
}
