use anyhow::Result;
use camino::Utf8PathBuf;
use structopt::StructOpt;

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::{CargoRunner, Runner};
use crate::utils::PKG_PROJECT_ROOT;

use std::str::FromStr;

#[derive(Debug, StructOpt)]
pub struct UnitTest {
    // The target to build Rover for
    #[structopt(long = "target", default_value, possible_values = &POSSIBLE_TARGETS)]
    pub(crate) target: Target,
}

impl UnitTest {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let mut cargo_runner = CargoRunner::new(verbose)?;
        cargo_runner.test(&self.target)?;

        let check_glibc_script = "./check_glibc.sh".to_string();
        let runner = Runner {
            verbose,
            tool_name: check_glibc_script.clone(),
            tool_exe: Utf8PathBuf::from_str(&check_glibc_script)?,
        };
        let bin_path = format!("./target/{}/debug/rover", &self.target);
        runner.exec(&[&bin_path], &PKG_PROJECT_ROOT, None)?;
        Ok(())
    }
}
