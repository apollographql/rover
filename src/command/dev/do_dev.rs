use dialoguer::Input;
use interprocess::local_socket::LocalSocketStream;
use saucer::Utf8PathBuf;
use tempdir::TempDir;

use super::compose::ComposeRunner;
use super::router::RouterRunner;
use super::socket::{MessageReceiver, MessageSender};
use super::{Dev, DevOpts};
use crate::command::dev::command::{CommandRunner, CommandRunnerMessage};
use crate::command::RoverOutput;
use crate::error::RoverError;
use crate::utils::client::StudioClientConfig;
use crate::Result;

use std::{sync::mpsc::sync_channel, time::Duration};

pub fn log_err_and_continue(err: RoverError) {
    let _ = err.print();
}

impl DevOpts {
    pub fn get_name(&self) -> Result<String> {
        if let Some(name) = self.name.as_ref().map(|s| s.to_string()) {
            Ok(name)
        } else {
            let dirname = std::env::current_dir()
                .ok()
                .and_then(|x| x.file_name().map(|x| x.to_string_lossy().to_string()));
            let mut input = Input::new();
            input.with_prompt("what is the name of this subgraph?");
            if let Some(dirname) = dirname {
                input.default(dirname);
            }
            let name: String = input.interact_text()?;
            Ok(name)
        }
    }
}

impl Dev {
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        // TODO: update the `4000` once you can change the port
        // if rover dev is extending a supergraph, it should be the graph ref instead
        let socket_addr = "/tmp/supergraph-4000.sock";

        let (original_command_message_sender, command_message_receiver) =
            CommandRunner::message_channel();
        rayon::spawn(move || {
            CommandRunner::new(socket_addr, command_message_receiver)
                .handle_command_runner_messages();
        });
        let kill_command_message_sender = original_command_message_sender.clone();
        let _ = ctrlc::set_handler(move || {
            let (kill_sender, kill_receiver) = CommandRunner::ready_channel();
            eprintln!("\nshutting down `rover dev`");
            kill_command_message_sender
                .send(CommandRunnerMessage::KillTasks {
                    ready_sender: kill_sender,
                })
                .unwrap();
            kill_receiver.recv().unwrap();
            std::process::exit(1);
        });

        let runner_command_message_sender = original_command_message_sender.clone();
        let run = move || {
            let name = self.opts.get_name()?;

            // read the subgraphs (and router) that are already running as a part of this `rover dev` instance
            let session_subgraphs = MessageSender::new(socket_addr).get_subgraphs();

            // dbg!(&session_subgraphs);

            // get a [`SubgraphRefresher`] that takes care of getting the schema for a single subgraph
            // either by polling the introspection endpoint or by watching the file system
            let mut subgraph_refresher = self.opts.schema_opts.get_subgraph_watcher(
                socket_addr,
                name,
                runner_command_message_sender.clone(),
                client_config
                    .get_builder()
                    .with_timeout(Duration::from_secs(2))
                    .build()?,
                session_subgraphs,
            )?;

            // create a temp directory for the composed supergraph
            let temp_dir = TempDir::new("subgraph")?;
            let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?;
            let supergraph_schema_path = temp_path.join("supergraph.graphql");

            let (ready_sender, ready_receiver) = sync_channel(1);

            // if we can't connect to the socket, we should start it and listen for incoming
            // subgraph events
            if LocalSocketStream::connect(socket_addr).is_err() {
                tracing::info!("connected to socket {}", &socket_addr);
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
                            router_runner
                                .kill_or_spawn(runner_command_message_sender, compose_receiver);
                        },
                    );
                });
            } else {
                ready_sender.send(()).unwrap();
            }

            // block the main thread until we are ready to receive
            // subgraph events
            // this happens immediately in child `rover dev` sessions
            // and after we bind to the socket in main `rover dev` sessions
            ready_receiver.recv().unwrap();

            // watch the subgraph for changes on the main thread
            subgraph_refresher.watch_subgraph()?;
            Ok(RoverOutput::EmptySuccess)
        };

        run().map_err(|e| {
            let (kill_sender, kill_receiver) = CommandRunner::ready_channel();
            let e = if let Err(e) =
                original_command_message_sender.send(CommandRunnerMessage::KillTasks {
                    ready_sender: kill_sender,
                }) {
                e.into()
            } else {
                e
            };
            if let Err(e) = kill_receiver.recv() {
                e.into()
            } else {
                e
            }
        })
    }
}
