use anyhow::{Context, Result};
use clap::Parser;

use crate::commands::version::RoverVersion;
use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::{CargoRunner, StripRunner};

#[derive(Debug, Parser)]
pub struct Dist {
    /// The target to build Rover for
    #[arg(long = "target", env = "XTASK_TARGET", default_value_t, value_parser = POSSIBLE_TARGETS)]
    pub(crate) target: Target,

    // The version to check out and compile, otherwise install a local build
    #[arg(long)]
    pub(crate) version: Option<RoverVersion>,
}

impl Dist {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        let bin_paths = cargo_runner.build(&self.target, true, self.version.as_ref())?;

        if !cfg!(windows) {
            for (bin_name, bin_path) in &bin_paths {
                let strip_runner = StripRunner::new(bin_path.clone(), verbose)?;
                strip_runner
                    .run()
                    .with_context(|| format!("Could not strip symbols from {}", &bin_name))?;
            }
        }

        Ok(())
    }
}
