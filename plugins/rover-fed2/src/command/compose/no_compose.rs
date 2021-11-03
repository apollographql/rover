use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Compose {}

impl Compose {
    pub fn run(&self) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!(
            "This version of rover-fed2 does not support this command."
        ))
    }
}
