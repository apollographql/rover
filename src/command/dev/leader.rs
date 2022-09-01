use crate::{
    command::dev::{compose::ComposeRunner, do_dev::log_err_and_continue, router::RouterRunner},
    error::RoverError,
    Result,
};
use apollo_federation_types::{
    build::SubgraphDefinition,
    config::{FederationVersion, SupergraphConfig},
};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use saucer::{anyhow, Context};
use std::{collections::HashMap, fmt::Debug, io::BufReader, sync::mpsc::SyncSender};

use crate::command::dev::protocol::*;

#[derive(Debug)]
pub struct MessageReceiver {
    subgraphs: HashMap<SubgraphKey, SubgraphSdl>,
    socket_addr: String,
    compose_runner: ComposeRunner,
    router_runner: RouterRunner,
}

impl MessageReceiver {
    pub fn new(
        socket_addr: &str,
        compose_runner: ComposeRunner,
        router_runner: RouterRunner,
    ) -> Result<Self> {
        if let Ok(stream) = LocalSocketStream::connect(socket_addr) {
            // write to the socket so we don't make the other session deadlock waiting on a message
            let mut stream = BufReader::new(stream);
            let _ = socket_write(&(), &mut stream);
            Err(RoverError::new(anyhow!(
                "there is already a main `rover dev` session"
            )))
        } else {
            Ok(Self {
                subgraphs: HashMap::new(),
                socket_addr: socket_addr.to_string(),
                compose_runner,
                router_runner,
            })
        }
    }

    pub fn supergraph_config(&self) -> SupergraphConfig {
        let mut supergraph_config: SupergraphConfig = self
            .subgraphs
            .iter()
            .map(|((name, url), sdl)| SubgraphDefinition::new(name, url.to_string(), sdl))
            .collect::<Vec<SubgraphDefinition>>()
            .into();
        supergraph_config.set_federation_version(FederationVersion::LatestFedTwo);
        supergraph_config
    }

    pub fn compose(&mut self, stream: &mut BufReader<LocalSocketStream>) {
        let composition_result = self
            .compose_runner
            .run(&mut self.supergraph_config())
            .map(|maybe_new_schema| {
                if maybe_new_schema.is_some() {
                    let _ = self.router_runner.spawn().map_err(log_err_and_continue);
                }
                maybe_new_schema
            })
            .map_err(|e| {
                eprintln!("{}", e);
                let _ = self.router_runner.kill().map_err(log_err_and_continue);
                e
            });
        let _ = socket_write(&composition_result, stream).map_err(log_err_and_continue);
    }

    pub fn add_subgraph(
        &mut self,
        subgraph_entry: &SubgraphEntry,
        stream: &mut BufReader<LocalSocketStream>,
    ) -> Result<()> {
        let ((name, url), sdl) = subgraph_entry;
        eprintln!("adding subgraph '{}'", &name);
        if self
            .subgraphs
            .get(&(name.to_string(), url.clone()))
            .is_some()
        {
            Err(RoverError::new(anyhow!(
                "subgraph with name '{}' and url '{}' already exists",
                &name,
                &url
            )))
        } else {
            self.subgraphs
                .insert((name.to_string(), url.clone()), sdl.to_string());
            self.compose(stream);
            Ok(())
        }
    }

    pub fn update_subgraph(
        &mut self,
        subgraph_entry: &SubgraphEntry,
        stream: &mut BufReader<LocalSocketStream>,
    ) -> Result<()> {
        let ((name, url), sdl) = subgraph_entry;
        eprintln!("updating subgraph '{}'", name);
        if let Some(prev_sdl) = self.subgraphs.get_mut(&(name.to_string(), url.clone())) {
            if prev_sdl != sdl {
                *prev_sdl = sdl.to_string();
                self.compose(stream);
            }
            Ok(())
        } else {
            self.add_subgraph(subgraph_entry, stream)
        }
    }

    pub fn remove_subgraph(
        &mut self,
        subgraph_name: &SubgraphName,
        stream: &mut BufReader<LocalSocketStream>,
    ) -> Result<()> {
        eprintln!("removing subgraph'{}'", &subgraph_name);
        let mut found = None;
        for (name, url) in self.subgraphs.keys() {
            if name == subgraph_name {
                found = Some((name, url));
                break;
            }
        }
        if let Some((name, url)) = found {
            self.subgraphs.remove(&(name.to_string(), url.clone()));
            self.compose(stream);
            Ok(())
        } else {
            Err(RoverError::new(anyhow!(
                "subgraph with name '{}' does not exist",
                &subgraph_name,
            )))
        }
    }

    pub fn receive_messages(&mut self, ready_sender: SyncSender<()>) -> Result<()> {
        let listener = LocalSocketListener::bind(&*self.socket_addr).with_context(|| {
            format!(
                "could not start local socket server at {}",
                &self.socket_addr
            )
        })?;
        ready_sender.send(()).unwrap();
        tracing::info!(
            "connected to socket {}, waiting for messages",
            &self.socket_addr
        );
        listener
            .incoming()
            .filter_map(handle_socket_error)
            .for_each(|stream| {
                let mut stream = BufReader::new(stream);
                tracing::info!("received incoming socket connection");
                let incoming = socket_read::<MessageKind>(&mut stream);
                tracing::debug!(?incoming);
                match incoming {
                    Ok(Some(message)) => match message {
                        MessageKind::AddSubgraph { subgraph_entry } => {
                            let _ = self
                                .add_subgraph(&subgraph_entry, &mut stream)
                                .map_err(log_err_and_continue);
                        }
                        MessageKind::UpdateSubgraph { subgraph_entry } => {
                            let _ = self
                                .update_subgraph(&subgraph_entry, &mut stream)
                                .map_err(log_err_and_continue);
                        }
                        MessageKind::RemoveSubgraph { subgraph_name } => {
                            let _ = self
                                .remove_subgraph(&subgraph_name, &mut stream)
                                .map_err(log_err_and_continue);
                        }
                        MessageKind::GetSubgraphs => {
                            eprintln!("notifying new `rover dev` session about existing subgraphs");
                            let _ = socket_write(&self.get_subgraphs(), &mut stream)
                                .map_err(log_err_and_continue);
                        }
                        MessageKind::KillRouter => {
                            let _ = self.router_runner.kill().map_err(log_err_and_continue);
                            let _ = socket_write(&(), &mut stream).map_err(log_err_and_continue);
                        }
                        MessageKind::HealthCheck => {
                            let _ = socket_write(&(), &mut stream).map_err(log_err_and_continue);
                        }
                    },
                    Err(e) => {
                        let _ = log_err_and_continue(e);
                    }
                    Ok(None) => {}
                }
            });
        Ok(())
    }

    pub fn get_subgraphs(&self) -> Vec<SubgraphKey> {
        self.subgraphs.keys().cloned().collect()
    }
}
