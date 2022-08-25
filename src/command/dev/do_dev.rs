use interprocess::local_socket::{LocalSocketStream, NameTypeSupport};
use saucer::{Context, Utf8PathBuf};
use tempdir::TempDir;

use super::compose::ComposeRunner;
use super::router::RouterRunner;
use super::socket::{MessageReceiver, MessageSender};
use super::Dev;
use crate::command::dev::socket::{socket_write, ComposeResult};
use crate::command::RoverOutput;
use crate::error::RoverError;
use crate::utils::client::StudioClientConfig;
use crate::Result;

use std::io::BufReader;
use std::{sync::mpsc::sync_channel, time::Duration};

pub fn log_err_and_continue(err: RoverError) {
    let _ = err.print();
}

impl Dev {
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        // TODO: update the `4000` once you can change the port
        // if rover dev is extending a supergraph, it should be the graph ref instead

        let socket_addr = {
            use NameTypeSupport::*;
            match NameTypeSupport::query() {
                OnlyPaths => "/tmp/supergraph-4000.sock",
                OnlyNamespaced | Both => "@supergraph-4000.sock",
            }
        };

        let name = self.opts.subgraph_opt.prompt_for_name()?;
        let kill_name = name.to_string();

        // read the subgraphs (and router) that are already running as a part of this `rover dev` instance
        let session_subgraphs = MessageSender::new(socket_addr, false).get_subgraphs();

        tracing::info!(
            "the main `rover dev` session currently has {} subgraphs",
            session_subgraphs.len() - 1
        );

        // check to see if the router is the only subgraph
        let is_main_session = session_subgraphs.len() == 1;

        // get a [`SubgraphRefresher`] that takes care of getting the schema for a single subgraph
        // either by polling the introspection endpoint or by watching the file system
        let mut subgraph_refresher = self.opts.schema_opts.get_subgraph_watcher(
            socket_addr,
            name,
            client_config
                .get_builder()
                .with_timeout(Duration::from_secs(2))
                .build()?,
            session_subgraphs,
            is_main_session,
        )?;

        // create a temp directory for the composed supergraph
        let temp_dir = TempDir::new("subgraph")?;
        let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?;
        let supergraph_schema_path = temp_path.join("supergraph.graphql");

        let (ready_sender, ready_receiver) = sync_channel(1);

        if let Ok(stream) = LocalSocketStream::connect(socket_addr) {
            // write to the socket so we don't make the other session deadlock waiting on a message
            let mut stream = BufReader::new(stream);
            let _ = socket_write(&(), &mut stream);
            ctrlc::set_handler(move || {
                eprintln!("\nshutting down subgraph '{}'", &kill_name);
                let _ = MessageSender::new(socket_addr, false).remove_subgraph(&kill_name);
                std::process::exit(1)
            })
            .context("could not set ctrl-c handler")?;
            ready_sender.send(()).unwrap();
        } else {
            // if we can't connect to the socket, we should start it and listen for incoming
            // subgraph events
            //
            // remove the socket file before starting in case it was here from last time
            // if we can't connect to it, it's safe to remove
            let _ = std::fs::remove_file(&socket_addr);

            // create a [`ComposeRunner`] that will be in charge of composing our supergraph
            let compose_runner = ComposeRunner::new(
                self.opts.plugin_opts.clone(),
                override_install_path.clone(),
                client_config.clone(),
                supergraph_schema_path.clone(),
            );

            // create a [`RouterRunner`] that we will spawn once we get our first subgraph
            // (which should come from this process but on another thread)
            let mut router_runner = RouterRunner::new(
                supergraph_schema_path,
                temp_path.join("config.yaml"),
                self.opts.plugin_opts.clone(),
                override_install_path,
                client_config,
            );

            // create a [`MessageReceiver`] that will keep track of the existing subgraphs
            let mut message_receiver = MessageReceiver::new(socket_addr, compose_runner)?;

            let (compose_sender, compose_receiver) = sync_channel(0);
            let kill_compose_sender = compose_sender.clone();
            ctrlc::set_handler(move || {
                eprintln!("\nshutting down main `rover dev` session");
                let _ = kill_compose_sender.send(ComposeResult::Kill);
                std::thread::sleep(Duration::from_secs(1));
                std::process::exit(1)
            })
            .context("could not set ctrl-c handler")?;
            rayon::spawn(move || {
                rayon::join(
                    // watch for subgraph updates coming in on the socket
                    // and send compose messages over the compose channel
                    || {
                        let _ = message_receiver
                            .receive_messages(ready_sender, compose_sender)
                            .map_err(log_err_and_continue);
                    },
                    move || {
                        router_runner.kill_or_spawn(compose_receiver);
                    },
                );
            });
        }

        // block the main thread until we are ready to receive
        // subgraph events
        // this happens immediately in child `rover dev` sessions
        // and after we bind to the socket in main `rover dev` sessions
        ready_receiver.recv().unwrap();

        // watch the subgraph for changes on the main thread
        subgraph_refresher.watch_subgraph()?;
        Ok(RoverOutput::EmptySuccess)
    }
}
