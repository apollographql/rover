use anyhow::Result;
use clap::Parser;

use crate::tools::NpmRunner;

#[derive(Debug, Parser)]
pub struct Lint {
    #[arg(long, short, action)]
    pub(crate) force: bool,
}

impl Lint {
    pub async fn run(&self) -> Result<()> {
        NpmRunner::new()?.lint()
    }
}
