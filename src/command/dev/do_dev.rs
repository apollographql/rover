use anyhow::{anyhow, Context};
use camino::Utf8PathBuf;
use rover_std::Emoji;

use super::protocol::{FollowerMessenger, LeaderSession};
use super::Dev;

use crate::command::dev::protocol::FollowerMessage;
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverOutput, RoverResult};

use crossbeam_channel::bounded as sync_channel;

pub fn log_err_and_continue(err: RoverError) -> RoverError {
    let _ = err.print();
    err
}

impl Dev {
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        self.opts
            .plugin_opts
            .prompt_for_license_accept(&client_config)?;
        let ipc_socket_addr = self.opts.supergraph_opts.ipc_socket_addr();

        let (follower_message_sender, follower_message_receiver) = sync_channel(0);
        let (leader_message_sender, leader_message_receiver) = sync_channel(0);

        if let Some(mut leader_session) = LeaderSession::new(
            &self.opts,
            override_install_path,
            &client_config,
            follower_message_sender.clone(),
            follower_message_receiver,
            leader_message_sender,
            leader_message_receiver.clone(),
        )? {
            let (ready_sender, ready_receiver) = sync_channel(1);
            let follower_messenger = FollowerMessenger::from_main_session(
                follower_message_sender.clone(),
                leader_message_receiver,
            );

            rayon::spawn(move || {
                ctrlc::set_handler(move || {
                    eprintln!(
                        "\n{}shutting down the `rover dev` session and all attached processes...",
                        Emoji::Stop
                    );
                    let _ = follower_message_sender
                        .send(FollowerMessage::shutdown(true))
                        .map_err(|e| {
                            let e =
                                RoverError::new(anyhow!("could not shut down router").context(e));
                            log_err_and_continue(e)
                        });
                })
                .context("could not set ctrl-c handler for main `rover dev` process")
                .unwrap();
            });

            rayon::spawn(move || {
                let _ = leader_session
                    .listen_for_all_subgraph_updates(ready_sender)
                    .map_err(log_err_and_continue);
            });

            ready_receiver.recv().unwrap();

            let mut subgraph_watcher = self.opts.subgraph_opts.get_subgraph_watcher(
                self.opts.supergraph_opts.router_socket_addr()?,
                &client_config,
                follower_messenger,
            )?;

            // watch for subgraph updates associated with the main `rover dev` process
            let _ = subgraph_watcher
                .watch_subgraph_for_changes()
                .map_err(log_err_and_continue);
        } else {
            // get a [`SubgraphRefresher`] that takes care of getting the schema for a single subgraph
            // either by polling the introspection endpoint or by watching the file system
            let mut subgraph_refresher = self.opts.subgraph_opts.get_subgraph_watcher(
                self.opts.supergraph_opts.router_socket_addr()?,
                &client_config,
                FollowerMessenger::from_attached_session(&ipc_socket_addr),
            )?;
            tracing::info!(
                "connecting to existing `rover dev` process running on `--supergraph-port {}`",
                &self.opts.supergraph_opts.supergraph_port
            );

            let health_messenger = FollowerMessenger::from_attached_session(&ipc_socket_addr);
            // start the interprocess socket health check in the background
            rayon::spawn(move || {
                let _ = health_messenger.health_check().map_err(|_| {
                    eprintln!("{}shutting down...", Emoji::Stop);
                    std::process::exit(1);
                });
            });

            // set up the ctrl+c handler to notify the main session to remove the killed subgraph
            let kill_messenger = FollowerMessenger::from_attached_session(&ipc_socket_addr);
            let kill_name = subgraph_refresher.get_name();
            ctrlc::set_handler(move || {
                eprintln!("\n{}shutting down...", Emoji::Stop);
                let _ = kill_messenger
                    .remove_subgraph(&kill_name)
                    .map_err(log_err_and_continue);
                std::process::exit(1);
            })
            .context("could not set ctrl-c handler")?;

            // watch for subgraph changes on the main thread
            // it will take care of updating the main `rover dev` session
            subgraph_refresher.watch_subgraph_for_changes()?;
        }

        unreachable!()
    }
}
