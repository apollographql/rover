use camino::Utf8PathBuf;

use crate::command::dev::runner::Runner;
use crate::utils::client::StudioClientConfig;
use crate::utils::supergraph_config::get_supergraph_config;
use crate::{RoverError, RoverOutput, RoverResult};

use super::router::RouterConfigHandler;
use super::Dev;

pub fn log_err_and_continue(err: RoverError) -> RoverError {
    let _ = err.print();
    err
}

impl Dev {
    pub async fn run(
        &self,
        // TODO: handle overriding
        _override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        self.opts
            .plugin_opts
            .prompt_for_license_accept(&client_config)?;

        let router_config_handler = RouterConfigHandler::try_from(&self.opts.supergraph_opts)?;

        let supergraph_config = get_supergraph_config(
            &self.opts.supergraph_opts.graph_ref,
            &self.opts.supergraph_opts.supergraph_config_path,
            self.opts.supergraph_opts.federation_version.as_ref(),
            client_config.clone(),
            &self.opts.plugin_opts.profile,
            false,
        )
        .await?;

        let dev_runner = Runner::new(
            self.opts.plugin_opts.clone(),
            &client_config,
            // FIXME: no clone if possible
            router_config_handler.clone(),
            // FIXME: actual error, no clone if possible
            supergraph_config
                .clone()
                .expect("supergraph_config is None; should be defined"),
        );

        dev_runner
            .run(
                &self.opts.supergraph_opts,
                &self.opts.subgraph_opts,
                &self.opts.plugin_opts,
                &client_config,
            )
            .await?;

        unreachable!("watch_subgraph_for_changes never returns");

        // FIXME: probably return to this
        //Ok(RoverOutput::EmptySuccess)
    }
}
