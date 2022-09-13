use saucer::{Context, Utf8PathBuf};
use tempdir::TempDir;

use super::compose::ComposeRunner;
use super::follower::FollowerMessenger;
use super::leader::LeaderMessenger;
use super::router::RouterRunner;
use super::Dev;

use crate::command::RoverOutput;
use crate::error::{anyhow, RoverError};
use crate::utils::client::StudioClientConfig;
use crate::Result;
use crate::Suggestion;

use std::{net::TcpListener, sync::mpsc::sync_channel, time::Duration};

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

        // read the subgraphs that are already running as a part of this `rover dev` instance
        let session_subgraph_finder = FollowerMessenger::new_subgraph(&ipc_socket_addr);
        session_subgraph_finder.version_check()?;
        let session_subgraphs = session_subgraph_finder.session_subgraphs()?;

        // get a [`SubgraphRefresher`] that takes care of getting the schema for a single subgraph
        // either by polling the introspection endpoint or by watching the file system
        let mut subgraph_refresher = self.opts.subgraph_opts.get_subgraph_watcher(
            &ipc_socket_addr,
            client_config
                .get_builder()
                .with_timeout(Duration::from_secs(2))
                .build()?,
            session_subgraphs,
            self.opts.supergraph_opts.router_socket_addr()?,
        )?;

        let is_main_session = subgraph_refresher.is_main_session();

        // create a temp directory for the composed supergraph
        let temp_dir = TempDir::new("subgraph")?;
        let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?;
        let supergraph_schema_path = temp_path.join("supergraph.graphql");

        let (ready_sender, ready_receiver) = sync_channel(1);

        if !is_main_session {
            tracing::info!(
                "connecting to existing `rover dev` session running on `--port {}`",
                &self.opts.supergraph_opts.port
            );
            let kill_sender = FollowerMessenger::new_subgraph(&ipc_socket_addr);
            let kill_name = subgraph_refresher.get_name();
            ctrlc::set_handler(move || {
                eprintln!("\nshutting down subgraph '{}'", &kill_name);
                let _ = kill_sender
                    .remove_subgraph(&kill_name)
                    .map_err(log_err_and_continue);
                std::process::exit(1)
            })
            .context("could not set ctrl-c handler")?;
            ready_sender.send("follower").unwrap();
        } else {
            tracing::info!("initializing main `rover dev session`");
            // if we can't connect to the socket, we should start it and listen for incoming
            // subgraph events
            //
            // remove the socket file before starting in case it was here from last time
            // if we can't connect to it, it's safe to remove
            let _ = std::fs::remove_file(&ipc_socket_addr);

            if TcpListener::bind(self.opts.supergraph_opts.router_socket_addr()?).is_err() {
                let mut err = RoverError::new(anyhow!(
                    "port {} is already in use",
                    &self.opts.supergraph_opts.port
                ));
                err.set_suggestion(Suggestion::Adhoc(
                    "try setting a different port for the router with the `--port` argument."
                        .to_string(),
                ));
                return Err(err);
            }

            // create a [`ComposeRunner`] that will be in charge of composing our supergraph
            let compose_runner = ComposeRunner::new(
                self.opts.plugin_opts.clone(),
                override_install_path.clone(),
                client_config.clone(),
                supergraph_schema_path.clone(),
            );

            // create a [`RouterRunner`] that we will spawn once we get our first subgraph
            // (which should come from this process but on another thread)
            let router_runner = RouterRunner::new(
                supergraph_schema_path,
                temp_path.join("config.yaml"),
                self.opts.plugin_opts.clone(),
                self.opts.supergraph_opts,
                override_install_path,
                client_config,
            );

            // create a [`MessageReceiver`] that will keep track of the existing subgraphs
            let mut message_receiver =
                LeaderMessenger::new(&ipc_socket_addr, compose_runner, router_runner)?;

            // attempt to install the router and supergraph plugins
            // before waiting for incoming messages

            message_receiver.install_plugins()?;

            let kill_messenger = FollowerMessenger::new_subgraph(&ipc_socket_addr);
            let kill_socket_addr = ipc_socket_addr.clone();
            ctrlc::set_handler(move || Self::shutdown(&kill_socket_addr, &kill_messenger))
                .context("could not set ctrl-c handler")?;

            rayon::spawn(move || {
                // watch for subgraph updates coming in on the socket
                // and send compose messages over the compose channel
                let _ = message_receiver
                    .receive_messages(ready_sender)
                    .map_err(log_err_and_continue);
            });
        }

        // block the main thread until we are ready to receive
        // subgraph events
        // this happens immediately in child `rover dev` sessions
        // and after we bind to the socket in main `rover dev` sessions
        ready_receiver.recv().unwrap();

        if !is_main_session {
            rayon::spawn(move || {
                let sender = FollowerMessenger::new_subgraph(&ipc_socket_addr);
                if let Err(e) = sender.health_check() {
                    log_err_and_continue(e);
                    std::process::exit(1);
                }
            })
        }

        // watch the subgraph for changes on the main thread
        subgraph_refresher.watch_subgraph()?;
        Ok(RoverOutput::EmptySuccess)
    }

    fn shutdown(ipc_socket_addr: &str, follower_messenger: &FollowerMessenger) {
        eprintln!("\nshutting down main `rover dev` session");
        let _ = follower_messenger.kill_router();
        let _ = std::fs::remove_file(ipc_socket_addr);
        std::process::exit(1)
    }
}
