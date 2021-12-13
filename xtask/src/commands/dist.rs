use anyhow::{Context, Result};
use structopt::StructOpt;

use crate::commands::version::RoverVersion;
use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::{CargoRunner, StripRunner};
use crate::utils::PKG_PROJECT_ROOT;

#[derive(Debug, StructOpt)]
pub struct Dist {
    /// The target to build Rover for
    #[structopt(long = "target", env = "XTASK_TARGET", default_value, possible_values = &POSSIBLE_TARGETS)]
    pub(crate) target: Target,

    // The version to check out and compile, otherwise install a local build
    #[structopt(long)]
    pub(crate) version: Option<RoverVersion>,
}

impl Dist {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let mut bin_paths = Vec::new();
        let mut cargo_runner = CargoRunner::new(verbose)?;
        bin_paths.push(
            cargo_runner
                .build_binary(&self.target, true, self.version.as_ref(), "rover")
                .with_context(|| "Could not build Rover.")?,
        );
        cargo_runner.set_path(PKG_PROJECT_ROOT.join("plugins").join("rover-fed2"));
        bin_paths.push(
            cargo_runner
                .build_binary(&self.target, true, self.version.as_ref(), "rover-fed2")
                .with_context(|| "Could not build rover-fed2")?,
        );

        if !cfg!(windows) {
            for bin_path in &bin_paths {
                let strip_runner = StripRunner::new(bin_path.clone(), verbose)?;
                strip_runner
                    .run()
                    .with_context(|| format!("Could not strip symbols from {}", &bin_path))?;
            }
        }

        Ok(())
    }
}
