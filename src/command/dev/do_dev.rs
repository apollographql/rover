use camino::Utf8PathBuf;
use rover_std::{infoln, warnln};

use crate::command::dev::runner::Runner;
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
        _override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        // Check for license acceptance.
        self.opts
            .plugin_opts
            .prompt_for_license_accept(&client_config)?;

        warnln!(
            "Do not run this command in production! It is intended for local development only."
        );

        let mut dev_runner = Runner::new(&client_config, &self.opts.supergraph_opts);

        infoln!("Starting main `rover dev` process");
        dev_runner.run(&self.opts.plugin_opts.profile).await?;

        Ok(RoverOutput::EmptySuccess)
    }
}
