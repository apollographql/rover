use std::time::Duration;

use dialoguer::Input;
use interprocess::local_socket::LocalSocketStream;
use saucer::Utf8PathBuf;
use tempdir::TempDir;

use super::command::CommandRunner;
use super::compose::ComposeRunner;
use super::router::RouterRunner;
use super::socket::{DevRunner, MessageSender};
use super::{Dev, DevOpts};
use crate::command::RoverOutput;
use crate::error::RoverError;
use crate::utils::client::StudioClientConfig;
use crate::Result;

pub fn handle_rover_error(err: RoverError) {
    if !format!("{:?}", &err).contains("EOF while parsing a value at line 1 column 0") {
        let _ = err.print();
    }
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
        let mut command_runner = CommandRunner::new(socket_addr);

        // read the subgraphs that are already running as a part of this `rover dev` instance
        let preexisting_endpoints = MessageSender::new(socket_addr)
            .get_subgraph_urls()
            .unwrap_or_else(|_| Vec::new());

        // get a [`SubgraphRefresher`] that takes care of getting the schema for a single subgraph
        // either by polling the introspection endpoint or by watching the file system
        let mut subgraph_refresher = self.opts.schema_opts.get_subgraph_watcher(
            socket_addr,
            name,
            &mut command_runner,
            client_config.get_reqwest_client(),
            preexisting_endpoints,
        )?;

        // watch the subgraph for changes on another thread
        rayon::spawn(move || {
            let _ = subgraph_refresher
                .watch_subgraph()
                .map_err(handle_rover_error);
        });

        // create a temp directory for the composed supergraph
        let temp_dir = TempDir::new("subgraph")?;
        let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?.join("supergraph.graphql");

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
                temp_path.clone(),
            );

            // create a [`RouterRunner`] that we will spawn once we get our first subgraph
            // (which should come from this process but on another thread)
            let router_runner = RouterRunner::new(
                temp_path,
                self.opts.plugin_opts.clone(),
                override_install_path,
                client_config,
            );

            // create a [`DevRunner`] that will keep track of the existing subgraphs
            let mut dev_runner =
                DevRunner::new(socket_addr, compose_runner, router_runner, command_runner)?;
            rayon::spawn(move || {
                let _ = dev_runner.receive_messages().map_err(handle_rover_error);
            });
        } else {
            let _ = ctrlc::set_handler(move || {
                command_runner.kill_tasks();
                std::process::exit(1);
            });
        }
        loop {
            std::thread::sleep(Duration::MAX)
        }
    }
}
