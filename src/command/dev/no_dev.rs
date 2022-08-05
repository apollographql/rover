use super::Dev;
use crate::{command::RoverOutput, error::RoverError, utils::client::StudioClientConfig, Result};
use saucer::{Utf8PathBuf, anyhow};

impl Dev {
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        Err(RoverError::new(anyhow!(
            "rover dev is not supported on this platform"
        )))
    }
}
