use crate::tools::Runner;
use crate::utils;

use anyhow::Result;
use camino::Utf8PathBuf;

pub(crate) struct GitRunner {
    rover_package_directory: Utf8PathBuf,
    runner: Runner,
}

impl GitRunner {
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let runner = Runner::new("git", verbose)?;
        let rover_package_directory = utils::project_root()?;

        Ok(GitRunner {
            runner,
            rover_package_directory,
        })
    }

    pub(crate) fn update_submodule(&self) -> Result<()> {
        unimplemented!()
    }

    pub(crate) fn remove_submodule(&self) -> Result<()> {
        unimplemented!()
    }
}
