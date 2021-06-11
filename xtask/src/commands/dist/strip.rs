use anyhow::Result;
use camino::Utf8PathBuf;

use crate::utils;

pub(crate) struct StripRunner {
    rover_executable: Utf8PathBuf,
    verbose: bool,
}

impl StripRunner {
    pub(crate) fn new(rover_executable: Utf8PathBuf, verbose: bool) -> Self {
        StripRunner {
            rover_executable,
            verbose,
        }
    }

    pub(crate) fn run(&self) -> Result<()> {
        let project_root = utils::project_root()?;
        let rover_executable = self.rover_executable.to_string();
        utils::exec(
            "strip",
            &[&rover_executable],
            &project_root,
            self.verbose,
            None,
        )?;
        Ok(())
    }
}
