use camino::Utf8PathBuf;
use rover_std::warnln;

use super::router::RouterConfigHandler;
use super::Dev;
use crate::command::dev::orchestrator::Orchestrator;
use crate::federation::supergraph_config::get_supergraph_config;
use crate::federation::Watcher;
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverResult};

pub fn log_err_and_continue(err: RoverError) -> RoverError {
    let _ = err.print();
    err
}

impl Dev {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<()> {
        self.opts
            .plugin_opts
            .prompt_for_license_accept(&client_config)?;

        let router_config_handler = RouterConfigHandler::try_from(&self.opts.supergraph_opts)?;
        let router_address = router_config_handler.get_router_address();

        let supergraph_config = get_supergraph_config(
            &self.opts.supergraph_opts.graph_ref,
            self.opts.supergraph_opts.supergraph_config_path.as_ref(),
            self.opts.supergraph_opts.federation_version.as_ref(),
            client_config.clone(),
            &self.opts.plugin_opts.profile,
        )
        .await?;
        let supergraph_config = if let Some(supergraph_config) = supergraph_config {
            supergraph_config
        } else {
            self.opts
                .subgraph_opts
                .get_single_subgraph_from_opts(router_address)?
        };

        let watcher = Watcher::new(
            supergraph_config,
            override_install_path.clone(),
            client_config.clone(),
            self.opts.plugin_opts.elv2_license_accepter,
            self.opts.plugin_opts.skip_update,
            &self.opts.plugin_opts.profile,
            self.opts.subgraph_opts.subgraph_polling_interval,
        )
        .await?;

        let orchestrator = Orchestrator::new(
            override_install_path,
            &client_config,
            self.opts.plugin_opts.clone(),
            watcher,
            router_config_handler,
            self.opts.supergraph_opts.license.clone(),
        )
        .await?;
        warnln!(
            "Do not run this command in production! It is intended for local development only."
        );

        orchestrator.run().await
    }
}
