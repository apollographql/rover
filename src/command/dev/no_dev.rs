use anyhow::anyhow;
use camino::Utf8PathBuf;
use rover_print::print::Print;
use timber::Level;

use crate::{
    RoverError, RoverOutput, RoverResult, command::dev::Dev, utils::client::StudioClientConfig,
};

impl Dev {
    pub async fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        _client_config: StudioClientConfig,
        _log_level: Option<Level>,
        _stderr: &impl Print,
    ) -> RoverResult<RoverOutput> {
        Err(RoverError::new(anyhow!(
            "rover dev is not supported on this platform"
        )))
    }
}
