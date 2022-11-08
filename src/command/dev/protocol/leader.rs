use crate::{
    command::dev::{
        compose::ComposeRunner, do_dev::log_err_and_continue, router::RouterRunner, DevOpts,
        DEV_COMPOSITION_VERSION,
    },
    utils::client::StudioClientConfig,
    RoverError, RoverErrorSuggestion, RoverResult, PKG_VERSION,
};
use anyhow::{anyhow, Context};
use apollo_federation_types::{
    build::SubgraphDefinition,
    config::{FederationVersion, SupergraphConfig},
};
use camino::Utf8PathBuf;
use crossbeam_channel::{Receiver, Sender};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use rover_std::Emoji;
use semver::Version;
use serde::{Deserialize, Serialize};
use tempdir::TempDir;

use std::{collections::HashMap, fmt::Debug, io::BufReader, net::TcpListener};

use super::{
    socket::{handle_socket_error, socket_read, socket_write},
    types::{
        CompositionResult, SubgraphEntry, SubgraphKey, SubgraphKeys, SubgraphName, SubgraphSdl,
    },
    FollowerMessage, FollowerMessageKind,
};

#[derive(Debug)]
pub struct LeaderSession {
    subgraphs: HashMap<SubgraphKey, SubgraphSdl>,
    ipc_socket_addr: String,
    compose_runner: ComposeRunner,
    router_runner: RouterRunner,
    follower_message_receiver: Receiver<FollowerMessage>,
    follower_message_sender: Sender<FollowerMessage>,
    leader_message_sender: Sender<LeaderMessageKind>,
    leader_message_receiver: Receiver<LeaderMessageKind>,
    federation_version: FederationVersion,
}

impl LeaderSession {
    /// Create a new [`LeaderSession`] that is responsible for running composition and the router
    /// It listens on a socket for incoming messages for subgraph changes, in addition to watching
    /// its own subgraph
    /// Returns:
    /// Ok(Some(Self)) when successfully initiated
    /// Ok(None) when a LeaderSession already exists for that address
    /// Err(RoverError) when something went wrong.
    pub fn new(
        opts: &DevOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: &StudioClientConfig,
        follower_message_sender: Sender<FollowerMessage>,
        follower_message_receiver: Receiver<FollowerMessage>,
        leader_message_sender: Sender<LeaderMessageKind>,
        leader_message_receiver: Receiver<LeaderMessageKind>,
    ) -> RoverResult<Option<Self>> {
        let ipc_socket_addr = opts.supergraph_opts.ipc_socket_addr();

        if let Ok(stream) = LocalSocketStream::connect(&*ipc_socket_addr) {
            // write to the socket so we don't make the other session deadlock waiting on a message
            let mut stream = BufReader::new(stream);
            socket_write(&FollowerMessage::health_check(false)?, &mut stream)?;
            let _ = LeaderSession::socket_read(&mut stream);
            // return early so an attached session can be created instead
            return Ok(None);
        }

        tracing::info!("initializing main `rover dev process`");
        // if we can't connect to the socket, we should start it and listen for incoming
        // subgraph events
        //
        // remove the socket file before starting in case it was here from last time
        // if we can't connect to it, it's safe to remove
        let _ = std::fs::remove_file(&ipc_socket_addr);

        let router_socket_addr = opts.supergraph_opts.router_socket_addr()?;

        if TcpListener::bind(router_socket_addr).is_err() {
            let mut err =
                RoverError::new(anyhow!("You cannot bind the router to '{}' because that address is already in use by another process on this machine.", &router_socket_addr));
            err.set_suggestion(RoverErrorSuggestion::Adhoc(
                format!("Try setting a different port for the router to bind to with the `--supergraph-port` argument, or shut down the process bound to '{}'.", &router_socket_addr)
            ));
            return Err(err);
        }

        // create a temp directory for the composed supergraph
        let temp_dir = TempDir::new("subgraph")?;
        let temp_path = Utf8PathBuf::try_from(temp_dir.into_path())?;
        let supergraph_schema_path = temp_path.join("supergraph.graphql");

        // create a [`ComposeRunner`] that will be in charge of composing our supergraph
        let mut compose_runner = ComposeRunner::new(
            opts.plugin_opts.clone(),
            override_install_path.clone(),
            client_config.clone(),
            supergraph_schema_path.clone(),
        );

        // create a [`RouterRunner`] that we will use to spawn the router when we have a successful composition
        let mut router_runner = RouterRunner::new(
            supergraph_schema_path,
            temp_path.join("config.yaml"),
            opts.plugin_opts.clone(),
            opts.supergraph_opts.router_socket_addr()?,
            override_install_path,
            client_config.clone(),
        );

        // install plugins before proceeding
        let federation_version = FederationVersion::ExactFedTwo(
            Version::parse(&DEV_COMPOSITION_VERSION)
                .map_err(|e| panic!("could not parse composition version:\n{:?}", e))
                .unwrap(),
        );

        router_runner.maybe_install_router()?;
        compose_runner.maybe_install_supergraph(federation_version.clone())?;

        Ok(Some(Self {
            subgraphs: HashMap::new(),
            ipc_socket_addr,
            compose_runner,
            router_runner,
            follower_message_receiver,
            follower_message_sender,
            leader_message_sender,
            leader_message_receiver,
            federation_version,
        }))
    }

    /// Start the session by watching for incoming subgraph updates and re-composing when needed
    pub fn listen_for_all_subgraph_updates(&mut self, ready_sender: Sender<()>) -> RoverResult<()> {
        self.receive_messages_from_attached_sessions()?;
        self.receive_all_subgraph_updates(ready_sender);
    }

    /// Listen for incoming subgraph updates and re-compose the supergraph
    fn receive_all_subgraph_updates(&mut self, ready_sender: Sender<()>) -> ! {
        ready_sender.send(()).unwrap();
        loop {
            tracing::trace!("main session waiting for follower message");
            let follower_message = self.follower_message_receiver.recv().unwrap();
            let leader_message = self.handle_follower_message_kind(follower_message.kind());

            if !follower_message.is_from_main_session() {
                leader_message.print();
            }
            let debug_message = format!("could not send message {:?}", &leader_message);
            tracing::trace!("main session sending leader message");

            self.leader_message_sender
                .send(leader_message)
                .expect(&debug_message);
            tracing::trace!("main session sent leader message");
        }
    }

    /// Listen on the socket for incoming [`FollowerMessageKind`] messages.
    fn receive_messages_from_attached_sessions(&self) -> RoverResult<()> {
        let listener = LocalSocketListener::bind(&*self.ipc_socket_addr).with_context(|| {
            format!(
                "could not start local socket server at {}",
                &self.ipc_socket_addr
            )
        })?;
        tracing::info!(
            "connected to socket {}, waiting for messages",
            &self.ipc_socket_addr
        );

        let follower_message_sender = self.follower_message_sender.clone();
        let leader_message_receiver = self.leader_message_receiver.clone();
        rayon::spawn(move || {
            listener
                .incoming()
                .filter_map(handle_socket_error)
                .for_each(|stream| {
                    let mut stream = BufReader::new(stream);
                    let follower_message = Self::socket_read(&mut stream);
                    let _ = match follower_message {
                        Ok(message) => {
                            let debug_message = format!("{:?}", &message);
                            tracing::debug!("the main `rover dev` process read a message from the socket, sending an update message on the channel");
                            follower_message_sender.send(message).unwrap_or_else(|_| {
                                panic!("failed to send message on channel: {}", &debug_message)
                            });
                            tracing::debug!("the main `rover dev` process is processing the message from the socket");
                            let leader_message = leader_message_receiver.recv().expect("failed to receive message on the channel");
                            tracing::debug!("the main `rover dev` process is sending the result on the socket");
                            Self::socket_write(leader_message, &mut stream)
                        }
                        Err(e) => {
                            tracing::debug!("the main `rover dev` process could not read incoming socket message, skipping channel update");
                            Err(e)
                        }
                    }.map_err(log_err_and_continue);
                });
        });

        Ok(())
    }

    /// Adds a subgraph to the internal supergraph representation.
    fn add_subgraph(&mut self, subgraph_entry: &SubgraphEntry) -> LeaderMessageKind {
        let is_first_subgraph = self.subgraphs.is_empty();
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
            self.subgraphs
                .insert((name.to_string(), url.clone()), sdl.to_string());
            let composition_result = self.compose();
            if let Err(composition_err) = composition_result {
                LeaderMessageKind::error(composition_err)
            } else if composition_result.transpose().is_some() && !is_first_subgraph {
                LeaderMessageKind::add_subgraph_composition_success(name)
            } else {
                LeaderMessageKind::MessageReceived
            }
        }
    }

    /// Updates a subgraph in the internal supergraph representation.
    fn update_subgraph(&mut self, subgraph_entry: &SubgraphEntry) -> LeaderMessageKind {
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
                    LeaderMessageKind::message_received()
                }
            } else {
                LeaderMessageKind::message_received()
            }
        } else {
            self.add_subgraph(subgraph_entry)
        }
    }

    /// Removes a subgraph from the internal subgraph representation.
    fn remove_subgraph(&mut self, subgraph_name: &SubgraphName) -> LeaderMessageKind {
        let found = self
            .subgraphs
            .keys()
            .find(|(name, _)| name == subgraph_name)
            .cloned();

        if let Some((name, url)) = found {
            self.subgraphs.remove(&(name.to_string(), url));
            let composition_result = self.compose();
            if let Err(composition_err) = composition_result {
                LeaderMessageKind::error(composition_err)
            } else if composition_result.transpose().is_some() {
                LeaderMessageKind::remove_subgraph_composition_success(&name)
            } else {
                LeaderMessageKind::message_received()
            }
        } else {
            LeaderMessageKind::message_received()
        }
    }

    /// Reruns composition, which triggers the router to reload.
    fn compose(&mut self) -> CompositionResult {
        self.compose_runner
            .run(&mut self.supergraph_config())
            .map(|maybe_new_schema| {
                if maybe_new_schema.is_some() {
                    let _ = self.router_runner.spawn().map_err(|e| panic!("{}", e));
                }
                maybe_new_schema
            })
            .map_err(|e| {
                let _ = self.router_runner.kill().map_err(log_err_and_continue);
                e
            })
    }

    /// Reads a [`FollowerMessage`] from an open socket connection.
    fn socket_read(stream: &mut BufReader<LocalSocketStream>) -> RoverResult<FollowerMessage> {
        socket_read(stream)
            .map(|message| {
                tracing::debug!("leader received message {:?}", &message);
                message
            })
            .map_err(|e| {
                e.context("the main `rover dev` process did not receive a valid incoming message")
                    .into()
            })
    }

    /// Writes a [`LeaderMessageKind`] to an open socket connection.
    fn socket_write(
        message: LeaderMessageKind,
        stream: &mut BufReader<LocalSocketStream>,
    ) -> RoverResult<()> {
        tracing::debug!("leader sending message {:?}", message);
        socket_write(&message, stream)
    }

    /// Gets the supergraph configuration from the internal state.
    /// Calling `.to_string()` on a [`SupergraphConfig`] writes
    fn supergraph_config(&self) -> SupergraphConfig {
        let mut supergraph_config: SupergraphConfig = self
            .subgraphs
            .iter()
            .map(|((name, url), sdl)| SubgraphDefinition::new(name, url.to_string(), sdl))
            .collect::<Vec<SubgraphDefinition>>()
            .into();
        supergraph_config.set_federation_version(self.federation_version.clone());
        supergraph_config
    }

    /// Gets the list of subgraphs running in this session
    fn get_subgraphs(&self) -> SubgraphKeys {
        tracing::debug!("notifying new `rover dev` process about existing subgraphs");
        self.subgraphs.keys().cloned().collect()
    }

    /// Shuts the router down, removes the socket file, and exits the process.
    pub fn shutdown(&mut self) {
        let _ = self.router_runner.kill().map_err(log_err_and_continue);
        let _ = std::fs::remove_file(&self.ipc_socket_addr);
        std::process::exit(1)
    }

    /// Handles a follower message by updating the internal subgraph representation if needed,
    /// and returns a [`LeaderMessageKind`] that can be sent over a socket or printed by the main session
    fn handle_follower_message_kind(
        &mut self,
        follower_message: &FollowerMessageKind,
    ) -> LeaderMessageKind {
        use FollowerMessageKind::*;
        match follower_message {
            AddSubgraph { subgraph_entry } => self.add_subgraph(subgraph_entry),

            UpdateSubgraph { subgraph_entry } => self.update_subgraph(subgraph_entry),

            RemoveSubgraph { subgraph_name } => self.remove_subgraph(subgraph_name),

            GetSubgraphs => LeaderMessageKind::current_subgraphs(self.get_subgraphs()),

            Shutdown => {
                self.shutdown();
                LeaderMessageKind::message_received()
            }

            HealthCheck => LeaderMessageKind::message_received(),

            GetVersion { follower_version } => LeaderMessageKind::get_version(follower_version),
        }
    }
}

impl Drop for LeaderSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum LeaderMessageKind {
    GetVersion {
        follower_version: String,
        leader_version: String,
    },
    LeaderSessionInfo {
        subgraphs: SubgraphKeys,
    },
    CompositionSuccess {
        action: String,
    },
    ErrorNotification {
        error: String,
    },
    MessageReceived,
}

impl LeaderMessageKind {
    pub fn get_version(follower_version: &str) -> Self {
        Self::GetVersion {
            follower_version: follower_version.to_string(),
            leader_version: PKG_VERSION.to_string(),
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
            action: format!("adding the '{}' subgraph", subgraph_name),
        }
    }

    pub fn update_subgraph_composition_success(subgraph_name: &SubgraphName) -> Self {
        Self::CompositionSuccess {
            action: format!("updating the '{}' subgraph", subgraph_name),
        }
    }

    pub fn remove_subgraph_composition_success(subgraph_name: &SubgraphName) -> Self {
        Self::CompositionSuccess {
            action: format!("removing the '{}' subgraph", subgraph_name),
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
                eprintln!("{}successfully composed after {}", Emoji::Success, &action);
            }
            LeaderMessageKind::LeaderSessionInfo { subgraphs } => {
                let subgraphs = match subgraphs.len() {
                    0 => "no subgraphs".to_string(),
                    1 => "1 subgraph".to_string(),
                    l => format!("{} subgraphs", l),
                };
                tracing::info!("the main `rover dev` process currently has {}", subgraphs);
            }
            LeaderMessageKind::GetVersion {
                leader_version,
                follower_version: _,
            } => {
                tracing::debug!(
                    "the main `rover dev` process is running version {}",
                    &leader_version
                );
            }
            LeaderMessageKind::MessageReceived => {
                tracing::debug!(
                        "the main `rover dev` process acknowledged the message, but did not take an action"
                    )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leader_message_can_get_version() {
        let follower_version = PKG_VERSION.to_string();
        let message = LeaderMessageKind::get_version(&follower_version);
        let expected_message_json = serde_json::to_string(&message).unwrap();
        assert_eq!(
            expected_message_json,
            serde_json::json!({
                "GetVersion": {
                    "follower_version": follower_version,
                    "leader_version": follower_version,
                }
            })
            .to_string()
        )
    }
}
