use super::Dev;
use crate::{command::RoverOutput, error::RoverError, utils::client::StudioClientConfig, Result};
use saucer::{anyhow, Parser};

impl Dev {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        Err(RoverError::new(anyhow!(
            "rover dev is not supported on this platform"
        )))
    }
}
