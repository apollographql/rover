use dialoguer::Input;
use interprocess::local_socket::LocalSocketStream;
use saucer::Utf8PathBuf;
use tempdir::TempDir;

use super::command::CommandRunner;
use super::compose::ComposeRunner;
use super::router::RouterRunner;
use super::socket::{MessageReceiver, MessageSender};
use super::{Dev, DevOpts};
use crate::command::RoverOutput;
use crate::error::RoverError;
use crate::utils::client::StudioClientConfig;
use crate::Result;

use std::sync::{Arc, Mutex};

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
        let name = self.opts.get_name()?;
        let command_runner = Arc::new(Mutex::new(CommandRunner::new(socket_addr)));

        // read the subgraphs that are already running as a part of this `rover dev` instance
        let session_subgraphs = MessageSender::new(socket_addr)
            .get_subgraphs()
            .unwrap_or_else(|_| Vec::new());

        // get a [`SubgraphRefresher`] that takes care of getting the schema for a single subgraph
        // either by polling the introspection endpoint or by watching the file system
        let mut subgraph_refresher = self.opts.schema_opts.get_subgraph_watcher(
            socket_addr,
            name,
            &mut Arc::clone(&command_runner).lock().unwrap(),
            client_config.get_reqwest_client(),
            session_subgraphs,
        )?;

        // create a temp directory for the composed supergraph
        let temp_dir = TempDir::new("subgraph")?;
        let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?;
        let supergraph_schema_path = temp_path.join("supergraph.graphql");
        // if we can't connect to the socket, we should start it and listen for incoming
        // subgraph events
        if LocalSocketStream::connect(socket_addr).is_err() {
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
            let router_runner = RouterRunner::new(
                supergraph_schema_path,
                temp_path.join("config.yaml"),
                self.opts.plugin_opts.clone(),
                override_install_path,
                client_config,
            );

            // create a [`MessageReceiver`] that will keep track of the existing subgraphs
            let mut message_receiver =
                MessageReceiver::new(socket_addr, compose_runner, router_runner)?;

            let command_runner_guard = Arc::clone(&command_runner);
            rayon::spawn(move || {
                let _ = message_receiver
                    .receive_messages(&mut command_runner_guard.lock().unwrap())
                    .map_err(log_err_and_continue);
                let _ = ctrlc::set_handler(move || {
                    command_runner_guard.lock().unwrap().kill_tasks();
                    std::process::exit(1);
                });
            });
        } else {
            let command_runner_guard = Arc::clone(&command_runner);
            let _ = ctrlc::set_handler(move || {
                command_runner_guard.lock().unwrap().kill_tasks();
                std::process::exit(1);
            });
        }
        // watch the subgraph for changes on the main thread
        subgraph_refresher.watch_subgraph()?;
        Ok(RoverOutput::EmptySuccess)
    }
}
