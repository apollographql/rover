use std::str::FromStr;
use std::{
    collections::{hash_map::Entry::Vacant, HashMap},
    fmt::Debug,
    io::BufReader,
    net::TcpListener,
};

use anyhow::{anyhow, Context};
use apollo_federation_types::{
    build::SubgraphDefinition,
    config::{FederationVersion, SupergraphConfig},
};
use camino::Utf8PathBuf;
use crossbeam_channel::{bounded, Receiver, Sender};
use futures::TryFutureExt;
use interprocess::local_socket::traits::{ListenerExt, Stream};
use interprocess::local_socket::ListenerOptions;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    command::dev::{
        compose::ComposeRunner,
        do_dev::log_err_and_continue,
        router::{RouterConfigHandler, RouterRunner},
        OVERRIDE_DEV_COMPOSITION_VERSION,
    },
    options::PluginOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverErrorSuggestion, RoverResult, PKG_VERSION,
};

use super::{
    create_socket_name,
    socket::{handle_socket_error, socket_read, socket_write},
    types::{
        CompositionResult, SubgraphEntry, SubgraphKey, SubgraphKeys, SubgraphName, SubgraphSdl,
    },
    FollowerChannel, FollowerMessage, FollowerMessageKind,
};

#[derive(Debug)]
pub struct LeaderSession {
    subgraphs: HashMap<SubgraphKey, SubgraphSdl>,
    raw_socket_name: String,
    compose_runner: ComposeRunner,
    router_runner: Option<RouterRunner>,
    follower_channel: FollowerChannel,
    leader_channel: LeaderChannel,
    federation_version: FederationVersion,
    supergraph_config: Option<SupergraphConfig>,
}

impl LeaderSession {
    /// Create a new [`LeaderSession`] that is responsible for running composition and the router
    /// It listens on a socket for incoming messages for subgraph changes, in addition to watching
    /// its own subgraph
    /// Returns:
    /// Ok(Some(Self)) when successfully initiated
    /// Ok(None) when a LeaderSession already exists for that address
    /// Err(RoverError) when something went wrong.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        override_install_path: Option<Utf8PathBuf>,
        client_config: &StudioClientConfig,
        leader_channel: LeaderChannel,
        follower_channel: FollowerChannel,
        plugin_opts: PluginOpts,
        supergraph_config: &Option<SupergraphConfig>,
        router_config_handler: RouterConfigHandler,
        license: Option<Utf8PathBuf>,
    ) -> RoverResult<Option<Self>> {
        let raw_socket_name = router_config_handler.get_raw_socket_name();
        let router_socket_addr = router_config_handler.get_router_address();
        let socket_name = create_socket_name(&raw_socket_name)?;

        if let Ok(stream) = Stream::connect(socket_name.clone()) {
            // write to the socket, so we don't make the other session deadlock waiting on a message
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
        let _ = std::fs::remove_file(&raw_socket_name);

        if TcpListener::bind(router_socket_addr).is_err() {
            let mut err =
                RoverError::new(anyhow!("You cannot bind the router to '{}' because that address is already in use by another process on this machine.", &router_socket_addr));
            err.set_suggestion(RoverErrorSuggestion::Adhoc(
                format!("Try setting a different port for the router to bind to with the `--supergraph-port` argument, or shut down the process bound to '{}'.", &router_socket_addr)
            ));
            return Err(err);
        }

        // create a [`ComposeRunner`] that will be in charge of composing our supergraph
        let mut compose_runner = ComposeRunner::new(
            plugin_opts.clone(),
            override_install_path.clone(),
            client_config.clone(),
            router_config_handler.get_supergraph_schema_path(),
        );

        // create a [`RouterRunner`] that we will use to spawn the router when we have a successful composition
        let mut router_runner = RouterRunner::new(
            router_config_handler.get_supergraph_schema_path(),
            router_config_handler.get_router_config_path(),
            plugin_opts.clone(),
            router_socket_addr,
            router_config_handler.get_router_listen_path(),
            override_install_path,
            client_config.clone(),
            license,
        );

        let config_fed_version = supergraph_config
            .clone()
            .and_then(|sc| sc.get_federation_version());

        let federation_version = Self::get_federation_version(
            config_fed_version,
            OVERRIDE_DEV_COMPOSITION_VERSION.clone(),
        )?;

        // install plugins before proceeding
        router_runner.maybe_install_router().await?;
        compose_runner
            .maybe_install_supergraph(federation_version.clone())
            .await?;

        router_config_handler.start()?;

        Ok(Some(Self {
            subgraphs: HashMap::new(),
            raw_socket_name,
            compose_runner,
            router_runner: Some(router_runner),
            follower_channel,
            leader_channel,
            federation_version,
            supergraph_config: supergraph_config.clone(),
        }))
    }

    /// Calculates what the correct version of Federation should be, based on the
    /// value of the given environment variable and the supergraph_schema
    ///
    /// The order of precedence is:
    /// Environment Variable -> Schema -> Default (Latest)
    fn get_federation_version(
        sc_config_version: Option<FederationVersion>,
        env_var: Option<String>,
    ) -> RoverResult<FederationVersion> {
        let env_var_version = if let Some(version) = env_var {
            match FederationVersion::from_str(&format!("={}", version)) {
                Ok(v) => Some(v),
                Err(e) => {
                    warn!("could not parse version from environment variable '{:}'", e);
                    info!("will check supergraph schema next...");
                    None
                }
            }
        } else {
            None
        };

        env_var_version.map(Ok).unwrap_or_else(|| {
            Ok(sc_config_version.unwrap_or_else(|| {
                warn!("federation version not found in supergraph schema");
                info!("using latest version instead");
                FederationVersion::LatestFedTwo
            }))
        })
    }

    /// Start the session by watching for incoming subgraph updates and re-composing when needed
    pub async fn listen_for_all_subgraph_updates(
        &mut self,
        ready_sender: futures::channel::mpsc::Sender<()>,
    ) -> RoverResult<()> {
        self.receive_messages_from_attached_sessions()?;
        self.receive_all_subgraph_updates(ready_sender).await;
        Ok(())
    }

    /// Listen for incoming subgraph updates and re-compose the supergraph
    async fn receive_all_subgraph_updates(
        &mut self,
        mut ready_sender: futures::channel::mpsc::Sender<()>,
    ) -> ! {
        ready_sender.try_send(()).unwrap();
        loop {
            tracing::trace!("main session waiting for follower message");
            let follower_message = self.follower_channel.receiver.recv().unwrap();
            let leader_message = self
                .handle_follower_message_kind(follower_message.kind())
                .await;

            if !follower_message.is_from_main_session() {
                leader_message.print();
            }
            let debug_message = format!("could not send message {:?}", &leader_message);
            tracing::trace!("main session sending leader message");

            self.leader_channel
                .sender
                .send(leader_message)
                .expect(&debug_message);
            tracing::trace!("main session sent leader message");
        }
    }

    /// Listen on the socket for incoming [`FollowerMessageKind`] messages.
    fn receive_messages_from_attached_sessions(&self) -> RoverResult<()> {
        let socket_name = create_socket_name(&self.raw_socket_name)?;
        let listener = ListenerOptions::new()
            .name(socket_name)
            .create_sync()
            .with_context(|| {
                format!(
                    "could not start local socket server at {:?}",
                    &self.raw_socket_name
                )
            })?;
        tracing::info!(
            "connected to socket {}, waiting for messages",
            &self.raw_socket_name
        );

        let follower_message_sender = self.follower_channel.sender.clone();
        let leader_message_receiver = self.leader_channel.receiver.clone();
        tokio::task::spawn_blocking(move || {
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
    async fn add_subgraph(&mut self, subgraph_entry: &SubgraphEntry) -> LeaderMessageKind {
        let is_first_subgraph = self.subgraphs.is_empty();
        let ((name, url), sdl) = subgraph_entry;

        if let Vacant(e) = self.subgraphs.entry((name.to_string(), url.clone())) {
            e.insert(sdl.to_string());

            // Followers add subgraphs, but sometimes those subgraphs depend on each other
            // (e.g., through extending a type in another subgraph). When that happens,
            // composition fails until _all_ subgraphs are loaded in. This acknowledges the
            // follower's message when we haven't loaded in all the subgraphs, deferring
            // composition until we have at least the number of subgraphs represented in the
            // supergraph.yaml file
            //
            // This applies only when the supergraph.yaml file is present. Without it, we will
            // try composition each time we add a subgraph
            if let Some(supergraph_config) = self.supergraph_config.clone() {
                let subgraphs_from_config = supergraph_config.into_iter();
                if self.subgraphs.len() < subgraphs_from_config.len() {
                    return LeaderMessageKind::MessageReceived;
                }
            }

            let composition_result = self.compose().await;
            if let Err(composition_err) = composition_result {
                LeaderMessageKind::error(composition_err)
            } else if composition_result.transpose().is_some() && !is_first_subgraph {
                LeaderMessageKind::add_subgraph_composition_success(name)
            } else {
                LeaderMessageKind::MessageReceived
            }
        } else {
            LeaderMessageKind::error(
                RoverError::new(anyhow!(
                    "subgraph with name '{}' and url '{}' already exists",
                    &name,
                    &url
                ))
                .to_string(),
            )
        }
    }

    /// Updates a subgraph in the internal supergraph representation.
    async fn update_subgraph(&mut self, subgraph_entry: &SubgraphEntry) -> LeaderMessageKind {
        let ((name, url), sdl) = &subgraph_entry;
        if let Some(prev_sdl) = self.subgraphs.get_mut(&(name.to_string(), url.clone())) {
            if prev_sdl != sdl {
                *prev_sdl = sdl.to_string();
                let composition_result = self.compose().await;
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
            self.add_subgraph(subgraph_entry).await
        }
    }

    /// Removes a subgraph from the internal subgraph representation.
    async fn remove_subgraph(&mut self, subgraph_name: &SubgraphName) -> LeaderMessageKind {
        let found = self
            .subgraphs
            .keys()
            .find(|(name, _)| name == subgraph_name)
            .cloned();

        if let Some((name, url)) = found {
            self.subgraphs.remove(&(name.to_string(), url));
            let composition_result = self.compose().await;
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

    /// Reruns composition
    async fn compose(&mut self) -> CompositionResult {
        match self
            .compose_runner
            .run(&mut self.supergraph_config_internal_representation())
            .and_then(|maybe_new_schema| async {
                if maybe_new_schema.is_some() {
                    if let Some(runner) = self.router_runner.as_mut() {
                        if let Err(err) = runner.spawn().await {
                            return Err(err.to_string());
                        }
                    }
                }
                Ok(maybe_new_schema)
            })
            .await
        {
            Ok(res) => Ok(res),
            Err(e) => {
                if let Some(runner) = self.router_runner.as_mut() {
                    let _ = runner.kill().await.map_err(log_err_and_continue);
                }
                Err(e)
            }
        }
    }

    /// Reads a [`FollowerMessage`] from an open socket connection.
    fn socket_read(
        stream: &mut BufReader<interprocess::local_socket::Stream>,
    ) -> RoverResult<FollowerMessage> {
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
        stream: &mut BufReader<interprocess::local_socket::Stream>,
    ) -> RoverResult<()> {
        tracing::debug!("leader sending message {:?}", message);
        socket_write(&message, stream)
    }

    /// Gets the supergraph configuration from the internal state. This can different from the
    /// supergraph.yaml file as it represents intermediate states of composition while adding
    /// subgraphs to the internal representation of that file
    fn supergraph_config_internal_representation(&self) -> SupergraphConfig {
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

    pub async fn shutdown(&mut self) {
        let router_runner = self.router_runner.take();
        let raw_socket_name = self.raw_socket_name.clone();
        if let Some(mut runner) = router_runner {
            let _ = runner.kill().await.map_err(log_err_and_continue);
        }
        let _ = std::fs::remove_file(&raw_socket_name);
        std::process::exit(1)
    }

    /// Handles a follower message by updating the internal subgraph representation if needed,
    /// and returns a [`LeaderMessageKind`] that can be sent over a socket or printed by the main session
    async fn handle_follower_message_kind(
        &mut self,
        follower_message: &FollowerMessageKind,
    ) -> LeaderMessageKind {
        use FollowerMessageKind::*;
        match follower_message {
            AddSubgraph { subgraph_entry } => self.add_subgraph(subgraph_entry).await,

            UpdateSubgraph { subgraph_entry } => self.update_subgraph(subgraph_entry).await,

            RemoveSubgraph { subgraph_name } => self.remove_subgraph(subgraph_name).await,

            GetSubgraphs => LeaderMessageKind::current_subgraphs(self.get_subgraphs()),

            Shutdown => {
                self.shutdown().await;
                LeaderMessageKind::message_received()
            }

            HealthCheck => LeaderMessageKind::message_received(),

            GetVersion { follower_version } => LeaderMessageKind::get_version(follower_version),
        }
    }
}

impl Drop for LeaderSession {
    fn drop(&mut self) {
        let router_runner = self.router_runner.take();
        let socket_addr = self.raw_socket_name.clone();
        tokio::task::spawn(async move {
            if let Some(mut runner) = router_runner {
                let _ = runner.kill().await.map_err(log_err_and_continue);
            }
            let _ = std::fs::remove_file(&socket_addr);
            std::process::exit(1)
        });
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
                eprintln!("successfully composed after {}", &action);
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

#[derive(Debug, Clone)]
pub struct LeaderChannel {
    pub sender: Sender<LeaderMessageKind>,
    pub receiver: Receiver<LeaderMessageKind>,
}

impl LeaderChannel {
    pub fn new() -> Self {
        let (sender, receiver) = bounded(0);

        Self { sender, receiver }
    }
}

#[cfg(test)]
mod tests {
    use apollo_federation_types::config::FederationVersion::{ExactFedOne, ExactFedTwo};
    use rstest::rstest;
    use semver::Version;
    use speculoos::assert_that;
    use speculoos::prelude::ResultAssertions;

    use super::*;

    #[rstest]
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

    #[rstest]
    #[case::env_var_no_yaml_fed_two(Some(String::from("2.3.4")), None, ExactFedTwo(Version::parse("2.3.4").unwrap()), false)]
    #[case::env_var_no_yaml_fed_one(Some(String::from("0.40.0")), None, ExactFedOne(Version::parse("0.40.0").unwrap()), false)]
    #[case::env_var_no_yaml_unsupported_fed_version(
        Some(String::from("1.0.1")),
        None,
        FederationVersion::LatestFedTwo,
        false
    )]
    #[case::nonsense_env_var_no_yaml(
        Some(String::from("crackers")),
        None,
        FederationVersion::LatestFedTwo,
        false
    )]
    #[case::env_var_with_yaml_fed_two(Some(String::from("2.3.4")), Some(ExactFedTwo(Version::parse("2.3.4").unwrap())), ExactFedTwo(Version::parse("2.3.4").unwrap()), false)]
    #[case::env_var_with_yaml_fed_one(Some(String::from("0.50.0")), Some(ExactFedTwo(Version::parse("2.3.5").unwrap())), ExactFedOne(Version::parse("0.50.0").unwrap()), false)]
    #[case::nonsense_env_var_with_yaml(Some(String::from("cheese")), Some(ExactFedTwo(Version::parse("2.3.5").unwrap())), ExactFedTwo(Version::parse("2.3.5").unwrap()), false)]
    #[case::yaml_no_env_var_fed_two(None, Some(ExactFedTwo(Version::parse("2.3.5").unwrap())),  ExactFedTwo(Version::parse("2.3.5").unwrap()), false)]
    #[case::yaml_no_env_var_fed_one(None, Some(ExactFedOne(Version::parse("0.69.0").unwrap())),  ExactFedOne(Version::parse("0.69.0").unwrap()), false)]
    #[case::nothing_grabs_latest(None, None, FederationVersion::LatestFedTwo, false)]
    fn federation_version_respects_precedence_order(
        #[case] env_var_value: Option<String>,
        #[case] config_value: Option<FederationVersion>,
        #[case] expected_value: FederationVersion,
        #[case] error_expected: bool,
    ) {
        let res = LeaderSession::get_federation_version(config_value, env_var_value);
        if error_expected {
            assert_that(&res).is_err();
        } else {
            assert_that(&res.unwrap()).is_equal_to(expected_value);
        }
    }
}
