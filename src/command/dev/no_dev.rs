use super::Dev;
use crate::{utils::client::StudioClientConfig, RoverError, RoverOutput, RoverResult};
use anyhow::anyhow;
use camino::Utf8PathBuf;
use crate::options::OutputOpts;

impl Dev {
    pub fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        _client_config: StudioClientConfig,
        _output_opts: &OutputOpts,
    ) -> RoverResult<RoverOutput> {
        Err(RoverError::new(anyhow!(
            "rover dev is not supported on this platform"
        )))
    }
}
