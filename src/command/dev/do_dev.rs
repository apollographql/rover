use camino::Utf8PathBuf;

use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverOutput, RoverResult};

use super::Dev;

pub fn log_err_and_continue(err: RoverError) -> RoverError {
    let _ = err.print();
    err
}

impl Dev {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        todo!()
    }
}
