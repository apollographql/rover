use saucer::Result;
use saucer::Utf8PathBuf;
use saucer::{clap, Parser};

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::{CargoRunner, Runner};
use crate::utils::PKG_PROJECT_ROOT;

use std::{env, str::FromStr};

#[derive(Debug, Parser)]
pub struct UnitTest {
    // The target to build Rover for
    #[clap(long = "target", env = "XTASK_TARGET", default_value_t, possible_values = &POSSIBLE_TARGETS)]
    pub(crate) target: Target,
}

impl UnitTest {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        cargo_runner.test(&self.target)?;

        if let Target::GnuLinux = self.target {
            if env::var_os("CHECK_GLIBC").is_some() {
                let check_glibc_script = "./check_glibc.sh".to_string();
                let runner = Runner {
                    verbose,
                    tool_name: check_glibc_script.clone(),
                    tool_exe: Utf8PathBuf::from_str(&check_glibc_script)?,
                };
                let bin_path = format!("./target/{}/debug/rover", &self.target);
                runner.exec(&[&bin_path], &PKG_PROJECT_ROOT, None)?;
            }
        }

        Ok(())
    }
}
