use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Smoke {
    #[arg(long = "binary_path")]
    pub(crate) binary_path: Option<Utf8PathBuf>,
}

impl Smoke {
    pub fn run(&self) -> anyhow::Result<()> {
        println!("Smokin'....");
        Ok(())
    }
}
