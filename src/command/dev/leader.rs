use crate::{
    command::dev::{
        compose::ComposeRunner, do_dev::log_err_and_continue, follower::FollowerMessageKind,
        router::RouterRunner, DEV_COMPOSITION_VERSION,
    },
    error::RoverError,
    Result, PKG_VERSION,
};
use apollo_federation_types::{
    build::SubgraphDefinition,
    config::{FederationVersion, SupergraphConfig},
};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use saucer::{anyhow, Context};
use semver::Version;
use serde::{Deserialize, Serialize};

use std::{collections::HashMap, fmt::Debug, io::BufReader, sync::mpsc::SyncSender};

use crate::command::dev::protocol::*;

#[derive(Debug)]
pub struct LeaderMessenger {
    subgraphs: HashMap<SubgraphKey, SubgraphSdl>,
    socket_addr: String,
    compose_runner: ComposeRunner,
    router_runner: RouterRunner,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum LeaderMessageKind {
    Version { version: String },
    LeaderSessionInfo { subgraphs: SubgraphKeys },
    CompositionSuccess { action: String },
    ErrorNotification { error: String },
    MessageReceived,
}

impl LeaderMessageKind {
    pub fn get_version() -> Self {
        Self::Version {
            version: PKG_VERSION.to_string(),
        }
    }

    pub fn current_subgraphs(subgraphs: SubgraphKeys) -> Self {
        Self::LeaderSessionInfo { subgraphs }
    }

    pub fn error(error: String) -> Self {
        Self::ErrorNotification { error }
    }

    pub fn add_subgraph_composition_success(subgraph_name: &SubgraphName) -> Self {
        Self::CompositionSuccess {
            action: format!("adding subgraph '{}'", subgraph_name),
        }
    }

    pub fn update_subgraph_composition_success(subgraph_name: &SubgraphName) -> Self {
        Self::CompositionSuccess {
            action: format!("updating subgraph '{}'", subgraph_name),
        }
    }

    pub fn remove_subgraph_composition_success(subgraph_name: &SubgraphName) -> Self {
        Self::CompositionSuccess {
            action: format!("removing subgraph '{}'", subgraph_name),
        }
    }

    pub fn message_received() -> Self {
        Self::MessageReceived
    }

    pub fn print(&self) {
        match self {
            LeaderMessageKind::ErrorNotification { error } => {
                eprintln!("{}", error);
            }
            LeaderMessageKind::CompositionSuccess { action } => {
                eprintln!("successfully re-composed after {}.", &action);
            }
            LeaderMessageKind::LeaderSessionInfo { subgraphs } => {
                tracing::info!(
                    "the main `rover dev` session currently has {} subgraphs",
                    subgraphs.len()
                );
            }
            LeaderMessageKind::Version { version } => {
                tracing::debug!(
                    "the main `rover dev` session is running version {}",
                    &version
                );
            }
            LeaderMessageKind::MessageReceived => {
                tracing::debug!(
                        "the main `rover dev` session acknowledged the message, but did not take an action"
                    )
            }
        }
    }
}

impl LeaderMessenger {
    pub fn new(
        socket_addr: &str,
        compose_runner: ComposeRunner,
        router_runner: RouterRunner,
    ) -> Result<Self> {
        if let Ok(stream) = LocalSocketStream::connect(socket_addr) {
            // write to the socket so we don't make the other session deadlock waiting on a message
            let mut stream = BufReader::new(stream);
            Self::socket_write(LeaderMessageKind::MessageReceived, &mut stream)?;
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

    fn socket_read(stream: &mut BufReader<LocalSocketStream>) -> Result<FollowerMessageKind> {
        tracing::debug!("leader reading message");
        let incoming = socket_read::<FollowerMessageKind>(stream);
        if let Ok(message) = &incoming {
            tracing::debug!("leader received message {:?}", message);
        } else {
            tracing::debug!("leader did not receive a message");
        }
        incoming
    }

    fn socket_write(
        message: LeaderMessageKind,
        stream: &mut BufReader<LocalSocketStream>,
    ) -> Result<()> {
        tracing::debug!("leader sending message {:?}", message);
        socket_write(&message, stream)
    }

    pub fn install_plugins(&mut self) -> Result<()> {
        self.router_runner.maybe_install_router()?;
        self.compose_runner
            .maybe_install_supergraph(&self.supergraph_config())?;
        Ok(())
    }

    pub fn supergraph_config(&self) -> SupergraphConfig {
        let mut supergraph_config: SupergraphConfig = self
            .subgraphs
            .iter()
            .map(|((name, url), sdl)| SubgraphDefinition::new(name, url.to_string(), sdl))
            .collect::<Vec<SubgraphDefinition>>()
            .into();
        supergraph_config.set_federation_version(FederationVersion::ExactFedTwo(
            Version::parse(DEV_COMPOSITION_VERSION)
                .map_err(|e| panic!("could not parse composition version:\n{:?}", e))
                .unwrap(),
        ));
        supergraph_config
    }

    fn compose(&mut self) -> CompositionResult {
        self.compose_runner
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
            })
    }

    pub fn add_subgraph(&mut self, subgraph_entry: SubgraphEntry) -> LeaderMessageKind {
        let ((name, url), sdl) = subgraph_entry;
        if self
            .subgraphs
            .get(&(name.to_string(), url.clone()))
            .is_some()
        {
            LeaderMessageKind::error(
                RoverError::new(anyhow!(
                    "subgraph with name '{}' and url '{}' already exists",
                    &name,
                    &url
                ))
                .to_string(),
            )
        } else {
            self.subgraphs.insert((name.to_string(), url), sdl);
            let composition_result = self.compose();
            if let Err(composition_err) = composition_result {
                LeaderMessageKind::error(composition_err)
            } else if composition_result.transpose().is_some() {
                LeaderMessageKind::add_subgraph_composition_success(&name)
            } else {
                LeaderMessageKind::MessageReceived
            }
        }
    }

    pub fn update_subgraph(&mut self, subgraph_entry: SubgraphEntry) -> LeaderMessageKind {
        let ((name, url), sdl) = &subgraph_entry;
        if let Some(prev_sdl) = self.subgraphs.get_mut(&(name.to_string(), url.clone())) {
            if prev_sdl != sdl {
                *prev_sdl = sdl.to_string();
                let composition_result = self.compose();
                if let Err(composition_err) = composition_result {
                    LeaderMessageKind::error(composition_err)
                } else if composition_result.transpose().is_some() {
                    LeaderMessageKind::update_subgraph_composition_success(name)
                } else {
                    LeaderMessageKind::MessageReceived
                }
            } else {
                LeaderMessageKind::MessageReceived
            }
        } else {
            self.add_subgraph(subgraph_entry)
        }
    }

    pub fn remove_subgraph(&mut self, subgraph_name: SubgraphName) -> LeaderMessageKind {
        let found = self
            .subgraphs
            .keys()
            .find(|(name, _)| name == &subgraph_name)
            .cloned();

        if let Some((name, url)) = found {
            self.subgraphs.remove(&(name.to_string(), url.clone()));
            let composition_result = self.compose();
            if let Err(composition_err) = composition_result {
                LeaderMessageKind::error(composition_err)
            } else if composition_result.transpose().is_some() {
                LeaderMessageKind::remove_subgraph_composition_success(&name)
            } else {
                LeaderMessageKind::MessageReceived
            }
        } else {
            LeaderMessageKind::message_received()
        }
    }

    pub fn receive_messages(&mut self, ready_sender: SyncSender<&str>) -> Result<()> {
        let listener = LocalSocketListener::bind(&*self.socket_addr).with_context(|| {
            format!(
                "could not start local socket server at {}",
                &self.socket_addr
            )
        })?;
        ready_sender.send("leader").unwrap();
        tracing::info!(
            "connected to socket {}, waiting for messages",
            &self.socket_addr
        );
        listener
            .incoming()
            .filter_map(handle_socket_error)
            .for_each(|stream| {
                let mut stream = BufReader::new(stream);
                let follower_message = Self::socket_read(&mut stream);
                let leader_message = match follower_message {
                    Ok(message) => match message {
                        FollowerMessageKind::AddSubgraph { subgraph_entry } => {
                            self.add_subgraph(subgraph_entry)
                        }

                        FollowerMessageKind::UpdateSubgraph { subgraph_entry } => {
                            self.update_subgraph(subgraph_entry)
                        }

                        FollowerMessageKind::RemoveSubgraph { subgraph_name } => {
                            self.remove_subgraph(subgraph_name)
                        }

                        FollowerMessageKind::GetSubgraphs => {
                            LeaderMessageKind::current_subgraphs(self.get_subgraphs())
                        }

                        FollowerMessageKind::KillRouter => {
                            let _ = self.router_runner.kill().map_err(log_err_and_continue);
                            LeaderMessageKind::message_received()
                        }
                        FollowerMessageKind::HealthCheck => LeaderMessageKind::message_received(),
                        FollowerMessageKind::GetVersion => LeaderMessageKind::get_version(),
                    },
                    Err(e) => {
                        let _ = log_err_and_continue(e);
                        let _ = self.router_runner.kill().map_err(log_err_and_continue);
                        LeaderMessageKind::message_received()
                    }
                };

                let _ =
                    Self::socket_write(leader_message, &mut stream).map_err(log_err_and_continue);
            });
        Ok(())
    }

    pub fn get_subgraphs(&self) -> SubgraphKeys {
        eprintln!("notifying new `rover dev` session about existing subgraphs");
        self.subgraphs.keys().cloned().collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_can_send_version_json() {
        let leader_message_kind = LeaderMessageKind::get_version();
        let json = serde_json::to_string(&leader_message_kind).unwrap();
        assert_eq!(serde_json::json!({}), json);
    }
}
