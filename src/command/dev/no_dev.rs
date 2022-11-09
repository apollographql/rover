use super::Dev;
use crate::{utils::client::StudioClientConfig, RoverError, RoverOutput, RoverResult};
use anyhow::anyhow;
use camino::Utf8PathBuf;

impl Dev {
    pub fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        _client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        Err(RoverError::new(anyhow!(
            "rover dev is not supported on this platform"
        )))
    }
}
