use anyhow::anyhow;
use camino::Utf8PathBuf;
use timber::Level;

use crate::command::dev::Dev;
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverOutput, RoverResult};

impl Dev {
    pub async fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        _client_config: StudioClientConfig,
        _log_level: Option<Level>,
    ) -> RoverResult<RoverOutput> {
        Err(RoverError::new(anyhow!(
            "rover dev is not supported on this platform"
        )))
    }
}
