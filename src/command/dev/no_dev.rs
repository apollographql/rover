use anyhow::anyhow;
use camino::Utf8PathBuf;

use crate::{
    command::dev::Dev, utils::client::StudioClientConfig, RoverError, RoverOutput, RoverResult,
};

impl Dev {
    pub async fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        _client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        Err(RoverError::new(anyhow!(
            "rover dev is not supported on this platform"
        )))
    }
}
