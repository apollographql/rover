use anyhow::{Context, Result};
use structopt::StructOpt;

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::{CargoRunner, StripRunner};

#[derive(Debug, StructOpt)]
pub struct Dist {
    #[structopt(long = "target", possible_values = &POSSIBLE_TARGETS)]
    target: Target,
}

impl Dist {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        let binary_path = cargo_runner
            .build(&self.target, true)
            .with_context(|| "Could not build Rover.")?;

        if !cfg!(windows) {
            let strip_runner = StripRunner::new(binary_path, verbose)?;
            strip_runner
                .run()
                .with_context(|| "Could not strip symbols from Rover's binary")?;
        }

        Ok(())
    }
}
