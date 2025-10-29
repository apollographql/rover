use anyhow::Result;
use clap::Parser;

use crate::tools::{CargoRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {
    #[arg(long, short, action)]
    pub(crate) force: bool,
}

impl Lint {
    pub async fn run(&self) -> Result<()> {
        CargoRunner::new()?.lint()?;
        NpmRunner::new()?.lint()
    }
}
