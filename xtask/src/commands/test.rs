use std::env;
use std::str::FromStr;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;

use crate::target::Target;
use crate::tools::{CargoRunner, Runner};
use crate::utils::PKG_PROJECT_ROOT;

#[derive(Debug, Parser)]
pub struct Test {
    // The target to build Rover for
    #[arg(long = "target", env = "XTASK_TARGET", default_value_t)]
    pub(crate) target: Target,
}

impl Test {
    pub fn run(&self) -> Result<()> {
        let cargo_runner = CargoRunner::new()?;
        cargo_runner.test(&self.target)?;

        if let Target::LinuxUnknownGnu = self.target {
            if env::var_os("CHECK_GLIBC").is_some() {
                let check_glibc_script = "./check_glibc.sh".to_string();
                let runner = Runner::new(Utf8PathBuf::from_str(&check_glibc_script)?.as_str());
                let bin_path = format!("./target/{}/debug/rover", &self.target);
                runner.exec(&[&bin_path], &PKG_PROJECT_ROOT, None)?;
            }
        }

        Ok(())
    }
}
