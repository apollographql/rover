use anyhow::Result;
use camino::Utf8PathBuf;

use crate::tools::Runner;
use crate::utils;

pub(crate) struct StripRunner {
    runner: Runner,
    rover_executable: Utf8PathBuf,
}

impl StripRunner {
    pub(crate) fn new(rover_executable: Utf8PathBuf, verbose: bool) -> Result<Self> {
        let runner = Runner::new("strip", verbose)?;
        Ok(StripRunner {
            runner,
            rover_executable,
        })
    }

    pub(crate) fn run(&self) -> Result<()> {
        let project_root = utils::project_root()?;
        self.runner
            .exec(&[&self.rover_executable.to_string()], &project_root, None)?;
        Ok(())
    }
}
