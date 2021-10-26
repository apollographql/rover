use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Compose {}

impl Compose {
    pub fn run(&self, _json: bool) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!(
            "This version of rover-fed does not support this command."
        ))
    }
}
