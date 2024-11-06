#![warn(missing_docs)]

use anyhow::anyhow;
use camino::Utf8PathBuf;

use crate::{
    command::Dev,
    utils::{client::StudioClientConfig, effect::read_file::FsReadFile},
    RoverError, RoverOutput, RoverResult,
};

use self::router::config::{RouterAddress, RunRouterConfig};

mod router;

impl Dev {
    /// Runs rover dev
    pub async fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        _client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        let read_file_impl = FsReadFile::default();
        let router_address = RouterAddress::new(
            self.opts.supergraph_opts.supergraph_address,
            self.opts.supergraph_opts.supergraph_port,
        );
        let _config = RunRouterConfig::default()
            .with_address(router_address)
            .with_config(
                &read_file_impl,
                self.opts.supergraph_opts.router_config_path.clone(),
            )
            .await
            .map_err(|err| RoverError::new(anyhow!("{}", err)))?;
        Ok(RoverOutput::EmptySuccess)
    }
}
