use anyhow::{anyhow, Context};
use camino::Utf8PathBuf;
use futures::channel::mpsc::channel;
use futures::future::join_all;
use futures::stream::StreamExt;
use futures::FutureExt;

use crate::command::dev::protocol::FollowerMessage;
use crate::utils::client::StudioClientConfig;
use crate::utils::supergraph_config::get_supergraph_config;
use crate::{RoverError, RoverOutput, RoverResult};

use super::protocol::{FollowerChannel, FollowerMessenger, LeaderChannel, LeaderSession};
use super::router::RouterConfigHandler;
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
        self.opts
            .plugin_opts
            .prompt_for_license_accept(&client_config)?;

        let router_config_handler = RouterConfigHandler::try_from(&self.opts.supergraph_opts)?;
        let router_address = router_config_handler.get_router_address();
        let leader_channel = LeaderChannel::new();
        let follower_channel = FollowerChannel::new();

        let supergraph_config = get_supergraph_config(
            &self.opts.supergraph_opts.graph_ref,
            &self.opts.supergraph_opts.supergraph_config_path,
            self.opts.supergraph_opts.federation_version.as_ref(),
            client_config.clone(),
            &self.opts.plugin_opts.profile,
            false,
        )
        .await?;

        let mut leader_session = LeaderSession::new(
            override_install_path,
            &client_config,
            leader_channel.clone(),
            follower_channel.clone(),
            self.opts.plugin_opts.clone(),
            &supergraph_config,
            router_config_handler,
            self.opts.supergraph_opts.license.clone(),
        )
        .await?
        // FIXME: use a rover error
        .expect("failed to get leader session");

        eprintln!("Do not run this command in production! It is intended for local development.");
        let (ready_sender, mut ready_receiver) = channel(1);
        let follower_messenger = FollowerMessenger::from_main_session(
            follower_channel.clone().sender,
            leader_channel.receiver,
        );

        tokio::task::spawn_blocking(move || {
            ctrlc::set_handler(move || {
                eprintln!("\nshutting down the `rover dev` session and all attached processes...");
                let _ = follower_channel
                    .sender
                    .send(FollowerMessage::shutdown(true))
                    .map_err(|e| {
                        let e = RoverError::new(anyhow!("could not shut down router").context(e));
                        log_err_and_continue(e)
                    });
            })
            .context("could not set ctrl-c handler for main `rover dev` process")
            .unwrap();
        });

        let subgraph_watcher_handle = tokio::task::spawn(async move {
            let _ = leader_session
                .listen_for_all_subgraph_updates(ready_sender)
                .await
                .map_err(log_err_and_continue);
        });

        ready_receiver.next().await.unwrap();

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
            .await
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

        let futs = subgraph_watchers.into_iter().map(|mut watcher| async move {
            let _ = watcher
                .watch_subgraph_for_changes(client_config.retry_period)
                .await
                .map_err(log_err_and_continue);
        });
        tokio::join!(join_all(futs), subgraph_watcher_handle.map(|_| ()));

        unreachable!("watch_subgraph_for_changes never returns")
    }
}
