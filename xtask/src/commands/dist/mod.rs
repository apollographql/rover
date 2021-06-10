mod strip;

use crate::commands::{CargoRunner, Target, POSSIBLE_TARGETS};

use anyhow::{Context, Result};
use structopt::StructOpt;

use crate::commands::dist::strip::StripRunner;

#[derive(Debug, StructOpt)]
pub struct Dist {
    #[structopt(long = "target", possible_values = &POSSIBLE_TARGETS)]
    target: Target,
}

impl Dist {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        let binary_path = cargo_runner
            .build(self.target.to_owned())
            .with_context(|| "Could not build Rover.")?;

        let strip_runner = StripRunner::new(binary_path, verbose);

        strip_runner
            .run()
            .with_context(|| "Could not strip symbols from Rover's binary")?;

        Ok(())
    }
}
