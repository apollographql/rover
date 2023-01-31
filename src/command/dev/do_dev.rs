use anyhow::{anyhow, Context};
use apollo_federation_types::build::SubgraphDefinition;
use camino::Utf8PathBuf;
use rover_std::Emoji;

use super::protocol::{FollowerChannel, FollowerMessenger, LeaderChannel, LeaderSession};
use super::router::RouterConfigHandler;
use super::Dev;

use crate::command::dev::protocol::FollowerMessage;
use crate::command::subgraph::{SubgraphListSubcommand, SubgraphFetchCommand};
use crate::options::{GraphRefOpt, SubgraphOpt};
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

        let router_config_handler = RouterConfigHandler::try_from(&self.opts.supergraph_opts)?;
        let router_address = router_config_handler.get_router_address()?;
        let ipc_socket_addr = router_config_handler.get_ipc_address()?;

        let mut initial_subgraphs = Vec::new();

        if let Some(graph_ref) = &self.opts.maybe_graph_ref {
            let output = SubgraphListSubcommand { graph: GraphRefOpt { graph_ref: graph_ref.clone() }, profile: self.opts.profile.clone() }.run(client_config.clone())?;
            if let RoverOutput::SubgraphList(response) = output {
                for subgraph in response.subgraphs {
                    let output = SubgraphFetchCommand { graph: GraphRefOpt { graph_ref: graph_ref.clone() }, profile: self.opts.profile.clone(), subgraph: SubgraphOpt {subgraph_name: subgraph.name.clone()} }.run(client_config.clone())?;
                    if let Some(subgraph_url) = subgraph.url {
                        if let RoverOutput::FetchResponse(response) = output {
                            initial_subgraphs.push(SubgraphDefinition::new(subgraph.name, subgraph_url, response.sdl.contents));
                        }
                    } else {
                        eprintln!("WARN: subgraph {name} does not have a routing url, you must publish one with `rover subgraph publish <GRAPH_REF> --name {name} --routing-url https://{name}.example.com`", name = subgraph.name);
                    }
                }
            }
        };

        dbg!(&initial_subgraphs);

        let leader_channel = LeaderChannel::new();
        let follower_channel = FollowerChannel::new();

        if let Some(mut leader_session) = LeaderSession::new(
            override_install_path,
            &client_config,
            leader_channel.clone(),
            follower_channel.clone(),
            self.opts.plugin_opts.clone(),
            self.opts.profile.clone(),
            router_config_handler,
        )? {
            let (ready_sender, ready_receiver) = sync_channel(1);
            let follower_messenger = FollowerMessenger::from_main_session(
                follower_channel.clone().sender,
                leader_channel.receiver,
            );

            rayon::spawn(move || {
                ctrlc::set_handler(move || {
                    eprintln!(
                        "\n{}shutting down the `rover dev` session and all attached processes...",
                        Emoji::Stop
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

            rayon::spawn(move || {
                let _ = leader_session
                    .listen_for_all_subgraph_updates(ready_sender)
                    .map_err(log_err_and_continue);
            });

            ready_receiver.recv().unwrap();

            let mut subgraph_watcher = self.opts.subgraph_opts.get_subgraph_watcher(
                router_address,
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
                router_address,
                &client_config,
                FollowerMessenger::from_attached_session(&ipc_socket_addr),
            )?;
            tracing::info!(
                "connecting to existing `rover dev` process by communicating via the interprocess socket located at {ipc_socket_addr}"
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
