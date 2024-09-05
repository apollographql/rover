use futures::channel::mpsc::channel;
use futures::future::join_all;
use futures::FutureExt;
use std::str::FromStr;
use std::{
    collections::{hash_map::Entry::Vacant, HashMap},
    fmt::Debug,
    io::BufReader,
};

use anyhow::{anyhow, Context};
use apollo_federation_types::build::SubgraphDefinition;
use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use futures::TryFutureExt;
use interprocess::local_socket::traits::ListenerExt;
use interprocess::local_socket::ListenerOptions;
use rover_std::{infoln, warnln};
use tracing::{info, warn};

use crate::command::dev::do_dev::log_err_and_continue;
use crate::command::dev::protocol::FollowerMessenger;
use crate::options::OptionalSubgraphOpts;
use crate::{
    command::dev::{
        compose::ComposeRunner,
        protocol::{create_socket_name, socket::handle_socket_error},
        router::{RouterConfigHandler, RouterRunner},
    },
    options::PluginOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverResult,
};

use super::protocol::socket::socket_read as protocol_socket_read;
use super::protocol::socket::socket_write as protocol_socket_write;
use super::protocol::{
    FollowerChannel, FollowerMessage, FollowerMessageKind, LeaderChannel, LeaderMessageKind,
};
use super::types::{
    CompositionResult, SubgraphEntry, SubgraphKey, SubgraphKeys, SubgraphName, SubgraphSdl,
};
use super::{SupergraphOpts, OVERRIDE_DEV_COMPOSITION_VERSION};

#[derive(Debug)]
pub struct Runner {
    compose_runner: ComposeRunner,
    router_runner: RouterRunner,
    router_config_handler: RouterConfigHandler,
    // WARNING: this field is just for ease of refactoring and we should think whether we want to
    // keep it
    supergraph_config: SupergraphConfig,
    // WARNING: this field is just for ease of refactoring and we should think whether we want to
    // keep it
    subgraphs: HashMap<SubgraphKey, SubgraphSdl>,
}

impl Runner {
    pub fn new(
        plugin_opts: PluginOpts,
        client_config: &StudioClientConfig,
        router_config_handler: RouterConfigHandler,
        supergraph_config: SupergraphConfig,
    ) -> Self {
        // Create a [`ComposeRunner`] that will be in charge of composing our supergraph
        let compose_runner = ComposeRunner::new(
            plugin_opts.clone(),
            None, // TODO: need to pass this.
            client_config.clone(),
            router_config_handler.get_supergraph_schema_path(),
        );

        // Create a [`RouterRunner`] that will be in charge of running the router
        let router_runner = RouterRunner::new(
            router_config_handler.get_supergraph_schema_path(),
            router_config_handler.get_router_config_path(),
            plugin_opts.clone(),
            router_config_handler.get_router_address(),
            router_config_handler.get_router_listen_path(),
            None, // TODO: need to pass this.
            client_config.clone(),
            None, // TODO: need to pass this.
        );

        Self {
            compose_runner,
            router_runner,
            router_config_handler,
            subgraphs: HashMap::new(),
            supergraph_config,
        }
    }

    pub async fn run(
        mut self,
        supergraph_opts: &SupergraphOpts,
        subgraph_opts: &OptionalSubgraphOpts,
        plugin_opts: &PluginOpts,
        client_config: &StudioClientConfig,
    ) -> RoverResult<()> {
        tracing::info!("initializing main `rover dev process`");
        warnln!(
            "Do not run this command in production! It is intended for local development only."
        );
        infoln!("Starting main `rover dev` process");

        // Install the necessary plugins if they're not already installed (supergraph binary and
        // the router binary)
        self.install_plugins().await?;

        // Start the watcher for the router config to hot reload the router when necessary
        self.router_config_handler.clone().start()?;

        let leader_channel = LeaderChannel::new();
        let follower_channel = FollowerChannel::new();
        let follower_messenger = FollowerMessenger::from_main_session(
            follower_channel.clone().sender,
            leader_channel.clone().receiver,
        );

        let subgraph_watchers = supergraph_opts
            .get_subgraph_watchers(
                client_config,
                // WARNING: we should probably make self.supergraph_config optional, if only for
                // the refactor's intermediate state
                // FIXME: no clone if possible
                Some(self.supergraph_config.clone()),
                follower_messenger.clone(),
                subgraph_opts.subgraph_polling_interval,
                &plugin_opts.profile,
                subgraph_opts.subgraph_retries,
            )
            .await
            .transpose()
            .unwrap_or_else(|| {
                subgraph_opts
                    .get_subgraph_watcher(
                        self.router_config_handler.get_router_address(),
                        client_config,
                        follower_messenger.clone(),
                    )
                    .map(|watcher| vec![watcher])
            })?;

        let (ready_sender, _ready_receiver) = channel(1);

        let subgraph_watcher_handle = tokio::task::spawn(async move {
            let _ = self
                .listen_for_all_subgraph_updates(ready_sender, &leader_channel, &follower_channel)
                .await
                .map_err(log_err_and_continue);
        });

        let futs = subgraph_watchers.into_iter().map(|mut watcher| async move {
            let _ = watcher
                .watch_subgraph_for_changes(client_config.retry_period)
                .await
                .map_err(log_err_and_continue);
        });

        tokio::join!(join_all(futs), subgraph_watcher_handle.map(|_| ()));

        Ok(())
    }

    /// Install the necessary plugins for composition (the supergraph binary) and the router (the
    /// router binary) if they're not already installed
    async fn install_plugins(&mut self) -> RoverResult<()> {
        // install plugins before proceeding
        self.router_runner.maybe_install_router().await?;
        self.compose_runner
            .maybe_install_supergraph(self.supergraph_config.get_federation_version().unwrap())
            .await?;

        RoverResult::Ok(())
    }

    pub async fn _watch_supergraph_config() -> RoverResult<()> {
        // TODO: set up supergraph watcher.
        // TODO: compose subgraph watchers from supergraph config.
        // let path = &self
        //     .opts
        //     .supergraph_opts
        //     .supergraph_config_path
        //     .as_ref()
        //     .unwrap()
        //     .to_path_buf()
        //     .unwrap();

        // let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // Fs::watch_file(path, tx);
        // tokio::spawn(async move {
        //     loop {
        //         let _ = rx.recv().await.unwrap();
        //         rover_std::infoln!("supergraph config updated");
        //     }
        // });
        //
        Ok(())
    }

    pub async fn listen_for_all_subgraph_updates(
        &mut self,
        ready_sender: futures::channel::mpsc::Sender<()>,
        leader_channel: &LeaderChannel,
        follower_channel: &FollowerChannel,
    ) -> RoverResult<()> {
        self.receive_messages_from_attached_sessions(leader_channel, follower_channel)?;
        self.receive_all_subgraph_updates(ready_sender, leader_channel, follower_channel)
            .await;
        Ok(())
    }

    /// Listen on the socket for incoming [`FollowerMessageKind`] messages.
    fn receive_messages_from_attached_sessions(
        &self,
        leader_channel: &LeaderChannel,
        follower_channel: &FollowerChannel,
    ) -> RoverResult<()> {
        let raw_socket_name = self.router_config_handler.get_raw_socket_name();

        let socket_name = create_socket_name(&raw_socket_name)?;

        let listener = ListenerOptions::new()
            .name(socket_name)
            .create_sync()
            .with_context(|| {
                format!(
                    "could not start local socket server at {:?}",
                    &raw_socket_name
                )
            })?;

        tracing::info!(
            "connected to socket {}, waiting for messages",
            &raw_socket_name
        );

        let follower_message_sender = follower_channel.sender.clone();
        let leader_message_receiver = leader_channel.receiver.clone();

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

    /// Reads a [`FollowerMessage`] from an open socket connection.
    fn socket_read(
        stream: &mut BufReader<interprocess::local_socket::Stream>,
    ) -> RoverResult<FollowerMessage> {
        protocol_socket_read(stream)
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
        protocol_socket_write(&message, stream)
    }

    /// Listen for incoming subgraph updates and re-compose the supergraph
    async fn receive_all_subgraph_updates(
        &mut self,
        mut ready_sender: futures::channel::mpsc::Sender<()>,
        leader_channel: &LeaderChannel,
        follower_channel: &FollowerChannel,
    ) -> ! {
        ready_sender.try_send(()).unwrap();
        loop {
            tracing::trace!("main session waiting for follower message");
            let follower_message = follower_channel.receiver.recv().unwrap();
            let leader_message = self
                .handle_follower_message_kind(follower_message.kind())
                .await;

            if !follower_message.is_from_main_session() {
                leader_message.print();
            }
            let debug_message = format!("could not send message {:?}", &leader_message);
            tracing::trace!("main session sending leader message");

            leader_channel
                .sender
                .send(leader_message)
                .expect(&debug_message);
            tracing::trace!("main session sent leader message");
        }
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
                todo!()
                //LeaderMessageKind::message_received(),}
            }

            HealthCheck => LeaderMessageKind::message_received(),

            GetVersion { follower_version } => LeaderMessageKind::get_version(follower_version),
        }
    }

    /// Gets the list of subgraphs running in this session
    fn get_subgraphs(&self) -> SubgraphKeys {
        tracing::debug!("notifying new `rover dev` process about existing subgraphs");
        self.subgraphs.keys().cloned().collect()
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

            // WARNING: in protocol/leader.rs, supergraph_config is optional; I've made it required
            // in this file for the Runner struct
            let subgraphs_from_config = self.supergraph_config.clone().into_iter();
            if self.subgraphs.len() < subgraphs_from_config.len() {
                return LeaderMessageKind::MessageReceived;
            }

            let composition_result = self.compose().await;
            if let Err(composition_err) = composition_result {
                // FIXME: actual error -> string, no bueno
                LeaderMessageKind::error(composition_err.to_string())
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
                    // FIXME: actual error -> string, no bueno
                    LeaderMessageKind::error(composition_err.to_string())
                    // WARNING: previous line below changed b/c of result/option switchup
                    //} else if composition_result.transpose().is_some() {
                } else if composition_result.is_ok() {
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
                // FIXME: actual error to string; should be actual error
                LeaderMessageKind::error(composition_err.to_string())
                // WARNING: switched to result, not option
            } else if composition_result.is_ok() {
                LeaderMessageKind::remove_subgraph_composition_success(&name)
            } else {
                LeaderMessageKind::message_received()
            }
        } else {
            LeaderMessageKind::message_received()
        }
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

        let federation_version = Self::get_federation_version(
            self.supergraph_config.get_federation_version(),
            OVERRIDE_DEV_COMPOSITION_VERSION.clone(),
            // FIXME: remove this and return a proper error; we might be down far enough that we'll
            // leave an orphaned router running or similar
        )
        .expect("failed to get federation version");

        supergraph_config.set_federation_version(federation_version);
        supergraph_config
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

    /// Reruns composition, which triggers the router to reload.
    async fn compose(&mut self) -> CompositionResult {
        match self
            .compose_runner
            .run(&mut self.supergraph_config_internal_representation())
            .and_then(|maybe_new_schema| async {
                if maybe_new_schema.is_some() {
                    if let Err(err) = self.router_runner.spawn().await {
                        return Err(err.to_string());
                    }
                }
                Ok(maybe_new_schema)
            })
            .await
        {
            Ok(res) => Ok(res),
            Err(e) => {
                let _ = self
                    .router_runner
                    .kill()
                    .await
                    .map_err(log_err_and_continue);

                Err(e)
            }
        }
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

    // multi-terminal use case?
    //#[rstest]
    //fn leader_message_can_get_version() {
    //    let follower_version = PKG_VERSION.to_string();
    //    let message = LeaderMessageKind::get_version(&follower_version);
    //    let expected_message_json = serde_json::to_string(&message).unwrap();
    //    assert_eq!(
    //        expected_message_json,
    //        serde_json::json!({
    //            "GetVersion": {
    //                "follower_version": follower_version,
    //                "leader_version": follower_version,
    //            }
    //        })
    //        .to_string()
    //    )
    //}

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
        let res = Runner::get_federation_version(config_value, env_var_value);
        if error_expected {
            assert_that(&res).is_err();
        } else {
            assert_that(&res.unwrap()).is_equal_to(expected_value);
        }
    }
}
