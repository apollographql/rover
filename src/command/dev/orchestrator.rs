use std::str::FromStr;
use std::{
    collections::{hash_map::Entry::Vacant, HashMap},
    fmt::Debug,
    net::TcpListener,
};

use anyhow::anyhow;
use apollo_federation_types::{
    config::{FederationVersion, SupergraphConfig},
    javascript::SubgraphDefinition,
};
use camino::Utf8PathBuf;
use futures::TryFutureExt;
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
    RoverError, RoverErrorSuggestion, RoverResult,
};

use super::protocol::{
    CompositionResult, SubgraphEntry, SubgraphKey, SubgraphMessage, SubgraphMessageChannel,
    SubgraphName, SubgraphSdl,
};

/// The top-level runner which handles router, recomposition of supergraphs, and wrangling the various `SubgraphWatcher`s
#[derive(Debug)]
pub(crate) struct Orchestrator {
    subgraphs: HashMap<SubgraphKey, SubgraphSdl>,
    compose_runner: ComposeRunner,
    router_runner: Option<RouterRunner>,
    subgraph_updates: SubgraphMessageChannel,
    federation_version: FederationVersion,
    supergraph_config: Option<SupergraphConfig>,
}

impl Orchestrator {
    /// Create a new [`LeaderSession`] that is responsible for running composition and the router
    /// It listens on a socket for incoming messages for subgraph changes, in addition to watching
    /// its own subgraph
    /// Returns:
    /// Ok(Some(Self)) when successfully initiated
    /// Ok(None) when a LeaderSession already exists for that address
    /// Err(RoverError) when something went wrong.
    pub async fn new(
        override_install_path: Option<Utf8PathBuf>,
        client_config: &StudioClientConfig,
        subgraph_updates: SubgraphMessageChannel,
        plugin_opts: PluginOpts,
        supergraph_config: &Option<SupergraphConfig>,
        router_config_handler: RouterConfigHandler,
        license: Option<Utf8PathBuf>,
    ) -> RoverResult<Self> {
        let raw_socket_name = router_config_handler.get_raw_socket_name();
        let router_socket_addr = router_config_handler.get_router_address();

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

        let config_fed_version = supergraph_config
            .clone()
            .and_then(|sc| sc.get_federation_version());

        let federation_version = Self::get_federation_version(
            config_fed_version,
            OVERRIDE_DEV_COMPOSITION_VERSION.clone(),
        )?;

        // create a [`ComposeRunner`] that will be in charge of composing our supergraph
        let compose_runner = ComposeRunner::new(
            plugin_opts.clone(),
            override_install_path.clone(),
            client_config.clone(),
            router_config_handler.get_supergraph_schema_path(),
            federation_version.clone(),
        )
        .await?;

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

        // install plugins before proceeding
        router_runner.maybe_install_router().await?;
        router_config_handler.start()?;

        Ok(Self {
            subgraphs: HashMap::new(),
            compose_runner,
            router_runner: Some(router_runner),
            subgraph_updates,
            federation_version,
            supergraph_config: supergraph_config.clone(),
        })
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

    /// Listen for incoming subgraph updates and re-compose the supergraph
    pub(crate) async fn receive_all_subgraph_updates(
        &mut self,
        mut ready_sender: futures::channel::mpsc::Sender<()>,
    ) -> ! {
        ready_sender.try_send(()).unwrap();
        loop {
            tracing::trace!("main session waiting for follower message");
            let message = self.subgraph_updates.receiver.recv().unwrap();
            self.handle_subgraph_message(&message).await;
        }
    }

    /// Adds a subgraph to the internal supergraph representation.
    async fn add_subgraph(&mut self, subgraph_entry: &SubgraphEntry) {
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
                    return;
                }
            }

            let composition_result = self.compose().await;
            if let Err(composition_err) = composition_result {
                eprintln!("{composition_err}");
            } else if composition_result.transpose().is_some() && !is_first_subgraph {
                eprintln!("successfully composed after adding the '{name}' subgraph");
            } else {
                return;
            }
        } else {
            eprintln!(
                "subgraph with name '{}' and url '{}' already exists",
                &name, &url
            );
        }
    }

    /// Updates a subgraph in the internal supergraph representation.
    async fn update_subgraph(&mut self, subgraph_entry: &SubgraphEntry) {
        let ((name, url), sdl) = &subgraph_entry;
        if let Some(prev_sdl) = self.subgraphs.get_mut(&(name.to_string(), url.clone())) {
            if prev_sdl != sdl {
                *prev_sdl = sdl.to_string();
                let composition_result = self.compose().await;
                if let Err(composition_err) = composition_result {
                    eprintln!("{composition_err}");
                } else if composition_result.transpose().is_some() {
                    eprintln!("successfully composed after updating the '{name}' subgraph");
                }
            }
        } else {
            self.add_subgraph(subgraph_entry).await
        }
    }

    // TODO: Call this function only from the supergraph file watcher
    /// Removes a subgraph from the internal subgraph representation.
    async fn remove_subgraph(&mut self, subgraph_name: &SubgraphName) {
        let found = self
            .subgraphs
            .keys()
            .find(|(name, _)| name == subgraph_name)
            .cloned();

        if let Some((name, url)) = found {
            self.subgraphs.remove(&(name.to_string(), url));
            let composition_result = self.compose().await;
            if let Err(composition_err) = composition_result {
                eprintln!("{composition_err}");
            } else if composition_result.transpose().is_some() {
                eprintln!("successfully composed after removing the '{name}' subgraph");
            }
        }
    }

    /// Reruns composition, which triggers the router to reload.
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

    /// Gets the supergraph configuration from the internal state. This can different from the
    /// supergraph.yaml file as it represents intermediate states of composition while adding
    /// subgraphs to the internal representation of that file
    fn supergraph_config_internal_representation(&self) -> SupergraphConfig {
        let mut supergraph_config: SupergraphConfig = self
            .subgraphs
            .iter()
            .map(|((name, url), sdl)| SubgraphDefinition {
                name: name.clone(),
                url: url.to_string(),
                sdl: sdl.clone(),
            })
            .collect::<Vec<SubgraphDefinition>>()
            .into();

        supergraph_config.set_federation_version(self.federation_version.clone());
        supergraph_config
    }

    /// Handles a follower message by updating the internal subgraph representation if needed,
    /// and returns a [`LeaderMessageKind`] that can be sent over a socket or printed by the main session
    async fn handle_subgraph_message(&mut self, message: &SubgraphMessage) {
        message.print();
        use SubgraphMessage::*;
        match message {
            Add { subgraph_entry } => self.add_subgraph(subgraph_entry).await,
            Update { subgraph_entry } => self.update_subgraph(subgraph_entry).await,
            Remove { subgraph_name } => self.remove_subgraph(subgraph_name).await,
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
        let res = Orchestrator::get_federation_version(config_value, env_var_value);
        if error_expected {
            assert_that(&res).is_err();
        } else {
            assert_that(&res.unwrap()).is_equal_to(expected_value);
        }
    }
}
