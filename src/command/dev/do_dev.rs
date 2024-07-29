use anyhow::{anyhow, Context};
use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use crossbeam_channel::bounded as sync_channel;

use rover_std::Fs;

use crate::command::dev::protocol::FollowerMessage;
use crate::command::supergraph::expand_supergraph_yaml;
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverOutput, RoverResult};

use super::protocol::{FollowerChannel, FollowerMessenger, LeaderChannel, LeaderSession};
use super::remote_subgraphs::RemoteSubgraphs;
use super::router::RouterConfigHandler;
use super::Dev;

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

        let router_config_handler = RouterConfigHandler::try_from(&self.opts.supergraph_opts)?;
        let router_address = router_config_handler.get_router_address();
        let raw_socket_name = router_config_handler.get_raw_socket_name();
        let leader_channel = LeaderChannel::new();
        let follower_channel = FollowerChannel::new();

        // Read in Remote subgraphs
        let remote_subgraphs = match &self.opts.supergraph_opts.graph_ref {
            Some(graph_ref) => Some(RemoteSubgraphs::fetch(
                &client_config.get_authenticated_client(&self.opts.plugin_opts.profile)?,
                &self
                    .opts
                    .supergraph_opts
                    .federation_version
                    .clone()
                    .unwrap_or(FederationVersion::LatestFedTwo),
                graph_ref,
            )?),
            None => None,
        };

        // Read in Local Supergraph Config
        let supergraph_config =
            if let Some(config_path) = &self.opts.supergraph_opts.supergraph_config_path {
                let config_content = Fs::read_file(config_path)?;
                Some(expand_supergraph_yaml(&config_content)?)
            } else {
                None
            };

        // Merge Remote and Local Supergraph Configs
        let supergraph_config = match remote_subgraphs {
            Some(remote_subgraphs) => match supergraph_config {
                Some(supergraph_config) => {
                    let mut merged_supergraph_config = remote_subgraphs.inner().clone();
                    merged_supergraph_config.merge_subgraphs(&supergraph_config);
                    Some(merged_supergraph_config)
                }
                None => Some(remote_subgraphs.inner().clone()),
            },
            None => supergraph_config,
        };

        // Build a Rayon Thread pool
        let tp = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .thread_name(|idx| format!("router-do-dev-{idx}"))
            .build()
            .map_err(|err| {
                RoverError::new(anyhow!("could not create router do dev thread pool: {err}",))
            })?;
        if let Some(mut leader_session) = LeaderSession::new(
            override_install_path,
            &client_config,
            leader_channel.clone(),
            follower_channel.clone(),
            self.opts.plugin_opts.clone(),
            &supergraph_config,
            router_config_handler,
        )? {
            eprintln!(
                "Do not run this command in production! It is intended for local development."
            );
            let (ready_sender, ready_receiver) = sync_channel(1);
            let follower_messenger = FollowerMessenger::from_main_session(
                follower_channel.clone().sender,
                leader_channel.receiver,
            );

            tp.spawn(move || {
                ctrlc::set_handler(move || {
                    eprintln!(
                        "\nshutting down the `rover dev` session and all attached processes..."
                    );
                    let _ = follower_channel
                        .sender
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

            let subgraph_watcher_handle = std::thread::spawn(move || {
                let _ = leader_session
                    .listen_for_all_subgraph_updates(ready_sender)
                    .map_err(log_err_and_continue);
            });

            ready_receiver.recv().unwrap();

            let subgraph_watchers = self
                .opts
                .supergraph_opts
                .get_subgraph_watchers(
                    &client_config,
                    supergraph_config,
                    follower_messenger.clone(),
                    self.opts.subgraph_opts.subgraph_polling_interval,
                    &self.opts.plugin_opts.profile,
                    self.opts.subgraph_opts.subgraph_retries,
                )
                .transpose()
                .unwrap_or_else(|| {
                    self.opts
                        .subgraph_opts
                        .get_subgraph_watcher(
                            router_address,
                            &client_config,
                            follower_messenger.clone(),
                        )
                        .map(|watcher| vec![watcher])
                })?;

            subgraph_watchers.into_iter().for_each(|mut watcher| {
                std::thread::spawn(move || {
                    let _ = watcher
                        .watch_subgraph_for_changes()
                        .map_err(log_err_and_continue);
                });
            });

            subgraph_watcher_handle
                .join()
                .expect("could not wait for subgraph watcher thread");
        } else {
            let follower_messenger = FollowerMessenger::from_attached_session(&raw_socket_name);
            let mut subgraph_refresher = self.opts.subgraph_opts.get_subgraph_watcher(
                router_address,
                &client_config,
                follower_messenger.clone(),
            )?;
            tracing::info!(
                "connecting to existing `rover dev` process by communicating via the interprocess socket located at {raw_socket_name}",
            );

            // start the interprocess socket health check in the background
            let health_messenger = follower_messenger.clone();
            tp.spawn(move || {
                let _ = health_messenger.health_check().map_err(|_| {
                    eprintln!("shutting down...");
                    std::process::exit(1);
                });
            });

            // set up the ctrl+c handler to notify the main session to remove the killed subgraph
            let kill_name = subgraph_refresher.get_name();
            ctrlc::set_handler(move || {
                eprintln!("\nshutting down...");
                let _ = follower_messenger
                    .remove_subgraph(&kill_name)
                    .map_err(log_err_and_continue);
                std::process::exit(1);
            })
            .context("could not set ctrl-c handler")?;

            // watch for subgraph changes on the main thread
            // it will take care of updating the main `rover dev` session
            subgraph_refresher.watch_subgraph_for_changes()?;
        }

        unreachable!("watch_subgraph_for_changes never returns")
    }
}
