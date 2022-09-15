use saucer::{Context, Utf8PathBuf};

use super::protocol::{FollowerMessenger, LeaderSession};
use super::Dev;

use crate::command::RoverOutput;
use crate::error::RoverError;
use crate::utils::client::StudioClientConfig;
use crate::Result;

use std::sync::mpsc::sync_channel;
use std::time::Duration;

pub fn log_err_and_continue(err: RoverError) -> RoverError {
    let _ = err.print();
    err
}

impl Dev {
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        self.opts
            .plugin_opts
            .prompt_for_license_accept(&client_config)?;
        let ipc_socket_addr = self.opts.supergraph_opts.ipc_socket_addr();

        if let Ok(mut leader_messenger) =
            LeaderSession::new(&self.opts, override_install_path, &client_config)
        {
            rayon::spawn(move || {
                // watch for subgraph updates coming in on the socket
                let _ = leader_messenger
                    .receive_messages()
                    .map_err(log_err_and_continue);
            });
        } else {
            // get a [`SubgraphRefresher`] that takes care of getting the schema for a single subgraph
            // either by polling the introspection endpoint or by watching the file system
            let mut subgraph_refresher = self.opts.subgraph_opts.get_subgraph_watcher(
                self.opts.supergraph_opts.router_socket_addr()?,
                client_config
                    .get_builder()
                    .with_timeout(Duration::from_secs(5))
                    .build()?,
                FollowerMessenger::from_attached_session(&ipc_socket_addr),
            )?;
            tracing::info!(
                "connecting to existing `rover dev` session running on `--port {}`",
                &self.opts.supergraph_opts.port
            );

            let health_messenger = FollowerMessenger::from_attached_session(&ipc_socket_addr);
            // start the interprocess socket health check in the background
            rayon::spawn(move || {
                let _ = health_messenger.health_check().map_err(|e| {
                    log_err_and_continue(e);
                    std::process::exit(1);
                });
            });

            // set up the ctrl+c handler to notify the main session to remove the killed subgraph
            let kill_messenger = FollowerMessenger::from_attached_session(&ipc_socket_addr);
            let kill_name = subgraph_refresher.get_name();
            ctrlc::set_handler(move || {
                let _ = kill_messenger
                    .remove_subgraph(&kill_name)
                    .map_err(log_err_and_continue);
                std::process::exit(1);
            })
            .context("could not set ctrl-c handler")?;

            // watch for subgraph changes on the main thread
            // it will take care of updating the main `rover dev` session
            subgraph_refresher.watch_subgraph()?;
        };

        unreachable!()
    }
}
