//! This module provides an object that can either produce a [`LazilyResolvedsupergraphConfig`] or a
//! [`FullyResolvedSupergraphConfig`] and uses the typestate pattern to enforce the order
//! in which certain steps must happen.
//!
//! The process that is outlined by this pattern is the following:
//!   1. Load remote subgraphs (if a [`GraphRef`] is provided)
//!   2. Load subgraphs from local config (if a supergraph config file is provided)
//!   3. Resolve subgraphs into one of: [`LazilyResolvedsupergraphConfig`] or [`FullyResolvedSupergraphConfig`]
//!      a. [`LazilyResolvedsupergraphConfig`] is used to spin up a [`SubgraphWatchers`] object, which
//!      provides SDL updates as subgraphs change
//!      b. [`FullyResolvedSupergraphConfig`] is used to produce a composition result
//!      from [`SupergraphBinary`]. This must be written to a file first, using the format defined
//!      by [`SupergraphConfig`]

use std::collections::BTreeMap;
use std::io::IsTerminal;

use anyhow::Context;
use apollo_federation_types::config::{
    ConfigError, FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
};
use camino::Utf8PathBuf;
use clap::error::ErrorKind as ClapErrorKind;
use clap::CommandFactory;
use dialoguer::Input;
use rover_client::shared::GraphRef;
use tower::{MakeService, Service, ServiceExt};
use tracing::warn;
use url::Url;

use self::fetch_remote_subgraph::FetchRemoteSubgraphFactory;
use self::fetch_remote_subgraphs::FetchRemoteSubgraphsRequest;
use super::error::ResolveSubgraphError;
use super::federation::{
    FederationVersionMismatch, FederationVersionResolver,
    FederationVersionResolverFromSupergraphConfig,
};
use super::full::introspect::ResolveIntrospectSubgraphFactory;
use super::full::FullyResolvedSupergraphConfig;
use super::lazy::LazilyResolvedSupergraphConfig;
use super::unresolved::UnresolvedSupergraphConfig;
use crate::cli::Rover;
use crate::utils::effect::read_stdin::ReadStdin;
use crate::utils::expansion::expand;
use crate::utils::parsers::FileDescriptorType;
use crate::RoverError;

pub mod fetch_remote_subgraph;
pub mod fetch_remote_subgraphs;
mod state;

/// This is a state-based resolver for the different stages of resolving a supergraph config
pub struct SupergraphConfigResolver<State> {
    state: State,
}

impl SupergraphConfigResolver<state::LoadRemoteSubgraphs> {
    /// Creates a new [`SupergraphConfigResolver`] using a target federation Version
    pub fn new(
        federation_version: FederationVersion,
    ) -> SupergraphConfigResolver<state::LoadRemoteSubgraphs> {
        SupergraphConfigResolver {
            state: state::LoadRemoteSubgraphs {
                federation_version_resolver: FederationVersionResolverFromSupergraphConfig::new(
                    federation_version,
                ),
            },
        }
    }
}

impl Default for SupergraphConfigResolver<state::LoadRemoteSubgraphs> {
    fn default() -> Self {
        SupergraphConfigResolver {
            state: state::LoadRemoteSubgraphs {
                federation_version_resolver: FederationVersionResolver::default(),
            },
        }
    }
}

/// Errors that may occur when loading remote subgraphs
#[derive(thiserror::Error, Debug)]
pub enum LoadRemoteSubgraphsError {
    /// Error captured by the underlying implementation of [`FetchRemoteSubgraphs`]
    #[error(transparent)]
    FetchRemoteSubgraphsError(Box<dyn std::error::Error + Send + Sync>),
}

impl SupergraphConfigResolver<state::LoadRemoteSubgraphs> {
    /// Optionally loads subgraphs from the Studio API using the contents of the `--graph-ref` flag
    /// and an implementation of [`FetchRemoteSubgraphs`]
    pub async fn load_remote_subgraphs<S>(
        self,
        mut fetch_remote_subgraphs_factory: S,
        graph_ref: Option<&GraphRef>,
    ) -> Result<SupergraphConfigResolver<state::LoadSupergraphConfig>, LoadRemoteSubgraphsError>
    where
        S: MakeService<
            (),
            FetchRemoteSubgraphsRequest,
            Response = BTreeMap<String, SubgraphConfig>,
        >,
        S::MakeError: std::error::Error + Send + Sync + 'static,
        S::Error: std::error::Error + Send + Sync + 'static,
    {
        if let Some(graph_ref) = graph_ref {
            let remote_subgraphs = fetch_remote_subgraphs_factory
                .make_service(())
                .await
                .map_err(|err| LoadRemoteSubgraphsError::FetchRemoteSubgraphsError(Box::new(err)))?
                .ready()
                .await
                .map_err(|err| LoadRemoteSubgraphsError::FetchRemoteSubgraphsError(Box::new(err)))?
                .call(FetchRemoteSubgraphsRequest::new(graph_ref.clone()))
                .await
                .map_err(|err| {
                    LoadRemoteSubgraphsError::FetchRemoteSubgraphsError(Box::new(err))
                })?;
            Ok(SupergraphConfigResolver {
                state: state::LoadSupergraphConfig {
                    federation_version_resolver: self.state.federation_version_resolver,
                    subgraphs: remote_subgraphs,
                },
            })
        } else {
            Ok(SupergraphConfigResolver {
                state: state::LoadSupergraphConfig {
                    federation_version_resolver: self.state.federation_version_resolver,
                    subgraphs: BTreeMap::default(),
                },
            })
        }
    }
}

/// Errors that may occur as a result of loading a local supergraph config
#[derive(thiserror::Error, Debug)]
pub enum LoadSupergraphConfigError {
    /// Occurs when a supergraph cannot be parsed as YAML
    #[error("Failed to parse the supergraph config. Error: {0}")]
    SupergraphConfig(ConfigError),
    /// IO error that occurs when the supergraph contents can't be access via File IO or Stdin
    #[error("Failed to read file descriptor. Error: {0}")]
    ReadFileDescriptor(RoverError),
    /// Occurs when a supergraph cannot be deserialised, ready for expansion
    #[error("Failed to deserialise the supergraph config. Error: {0}")]
    DeserializationError(#[from] serde_yaml::Error),
    /// Occurs when a supergraph cannot be expanded correctly
    #[error("Failed to expand supergraph config. Error: {0}")]
    ExpansionError(RoverError),
}

impl SupergraphConfigResolver<state::LoadSupergraphConfig> {
    /// Optionally loads the file from a specified [`FileDescriptorType`], using the implementation
    /// of [`ReadStdin`] in cases where `file_descriptor_type` is specified and points at stdin
    pub fn load_from_file_descriptor(
        self,
        read_stdin_impl: &mut impl ReadStdin,
        file_descriptor_type: Option<&FileDescriptorType>,
    ) -> Result<SupergraphConfigResolver<state::DefineDefaultSubgraph>, LoadSupergraphConfigError>
    {
        if let Some(file_descriptor_type) = file_descriptor_type {
            let supergraph_config =
                Self::get_supergraph_config(read_stdin_impl, file_descriptor_type)?;
            let origin_path = match file_descriptor_type {
                FileDescriptorType::File(file) => Some(file.clone()),
                FileDescriptorType::Stdin => None,
            };
            let federation_version_resolver = self
                .state
                .federation_version_resolver
                .from_supergraph_config(Some(&supergraph_config));
            let mut merged_subgraphs = self.state.subgraphs;
            for (name, subgraph_config) in supergraph_config.into_iter() {
                let subgraph_config = SubgraphConfig {
                    routing_url: subgraph_config.routing_url.or_else(|| {
                        merged_subgraphs
                            .get(&name)
                            .and_then(|remote_config| remote_config.routing_url.clone())
                    }),
                    schema: subgraph_config.schema,
                };
                merged_subgraphs.insert(name, subgraph_config);
            }
            Ok(SupergraphConfigResolver {
                state: state::DefineDefaultSubgraph {
                    origin_path,
                    federation_version_resolver,
                    subgraphs: merged_subgraphs,
                },
            })
        } else {
            Ok(SupergraphConfigResolver {
                state: state::DefineDefaultSubgraph {
                    origin_path: None,
                    federation_version_resolver: self
                        .state
                        .federation_version_resolver
                        .from_supergraph_config(None),
                    subgraphs: self.state.subgraphs,
                },
            })
        }
    }

    fn get_supergraph_config(
        read_stdin_impl: &mut impl ReadStdin,
        file_descriptor_type: &FileDescriptorType,
    ) -> Result<SupergraphConfig, LoadSupergraphConfigError> {
        let contents = file_descriptor_type
            .read_file_descriptor("supergraph config", read_stdin_impl)
            .map_err(LoadSupergraphConfigError::ReadFileDescriptor)?;
        let yaml_contents = expand(serde_yaml::from_str(&contents)?)
            .map_err(LoadSupergraphConfigError::ExpansionError)?;
        match SupergraphConfig::new_from_yaml(&(serde_yaml::to_string(&yaml_contents)?)) {
            Ok(supergraph_config) => Ok(supergraph_config),
            Err(err) => {
                warn!("Could not initially parse supergraph config: {}", err);
                warn!("Proceeding with empty supergraph config");
                Ok(SupergraphConfig::new(BTreeMap::new(), None))
            }
        }
    }
}

impl SupergraphConfigResolver<state::DefineDefaultSubgraph> {
    /// Prompts the user for subgraphs if they have not provided any so far
    pub fn define_default_subgraph_if_empty(
        mut self,
        default_subgraph: DefaultSubgraphDefinition,
    ) -> Result<SupergraphConfigResolver<state::ResolveSubgraphs>, ResolveSubgraphError> {
        if self.state.subgraphs.is_empty() {
            let subgraph_url = default_subgraph.url()?;
            let subgraph_name = default_subgraph.name()?;
            let subgraph_schema_path = default_subgraph.schema_path();

            let schema_source = match subgraph_schema_path {
                Some(subgraph_schema_path) => SchemaSource::File {
                    file: subgraph_schema_path.into_std_path_buf(),
                },
                None => SchemaSource::SubgraphIntrospection {
                    subgraph_url: subgraph_url.clone(),
                    introspection_headers: None,
                },
            };

            self.state.subgraphs.insert(
                subgraph_name,
                SubgraphConfig {
                    routing_url: Some(subgraph_url.to_string()),
                    schema: schema_source,
                },
            );
        } else {
            tracing::warn!("Attempting to define a default subgraph when the existing subgraph set is not empty");
        }
        Ok(SupergraphConfigResolver {
            state: state::ResolveSubgraphs {
                origin_path: self.state.origin_path,
                federation_version_resolver: self.state.federation_version_resolver,
                subgraphs: self.state.subgraphs,
            },
        })
    }

    /// Skips prompting the user for subgraphs if they have not provided any so far
    pub fn skip_default_subgraph(self) -> SupergraphConfigResolver<state::ResolveSubgraphs> {
        SupergraphConfigResolver {
            state: state::ResolveSubgraphs {
                origin_path: self.state.origin_path,
                federation_version_resolver: self.state.federation_version_resolver,
                subgraphs: self.state.subgraphs,
            },
        }
    }
}

/// Errors that may occur while resolving a supergraph config
#[derive(thiserror::Error, Debug)]
pub enum ResolveSupergraphConfigError {
    /// Occurs when the caller neither loads a remote supergraph config nor a local one
    #[error("No source found for supergraph config")]
    NoSource,
    /// Occurs when supergraph resolution is attempted without a supergraph root
    #[error("Unable to resolve supergraph config. Supergraph config root is missing")]
    MissingSupergraphConfigRoot,
    /// Occurs when the underlying resolver strategy can't resolve one or more
    /// of the subgraphs described in the supergraph config
    #[error(
        "Unable to resolve subgraphs.\n{}",
        ::itertools::join(.0.iter().map(|(n, e)| format!("{}: {}", n, e)), "\n")
    )]
    ResolveSubgraphs(BTreeMap<String, ResolveSubgraphError>),
    /// Occurs when the user-selected `FederationVersion` is within Federation 1 boundaries, but the
    /// subgraphs use the `@link` directive, which requires Federation 2
    #[error(transparent)]
    FederationVersionMismatch(#[from] FederationVersionMismatch),
    /// Occurs when a `FederationVersionResolver` was not supplied to an `UnresolvedSupergraphConfig`
    /// and federation version resolution was attempted
    #[error("Unable to resolve federation version")]
    MissingFederationVersionResolver,
}

/// Public alias for [`SupergraphConfigResolver<ResolveSubgraphs>`]
/// This state of [`SupergraphConfigResolver`] is ready to resolve subgraphs fully or lazily
pub type InitializedSupergraphConfigResolver = SupergraphConfigResolver<state::ResolveSubgraphs>;

impl SupergraphConfigResolver<state::ResolveSubgraphs> {
    /// Fully resolves the subgraph configurations in the supergraph config file to their SDLs
    pub async fn fully_resolve_subgraphs(
        &self,
        resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        supergraph_config_root: &Utf8PathBuf,
    ) -> Result<
        (
            FullyResolvedSupergraphConfig,
            BTreeMap<String, ResolveSubgraphError>,
        ),
        ResolveSupergraphConfigError,
    > {
        let unresolved_supergraph_config = UnresolvedSupergraphConfig::builder()
            .subgraphs(self.state.subgraphs.clone())
            .federation_version_resolver(self.state.federation_version_resolver.clone())
            .build();
        let resolved_supergraph_config = FullyResolvedSupergraphConfig::resolve(
            resolve_introspect_subgraph_factory,
            fetch_remote_subgraph_factory,
            supergraph_config_root,
            unresolved_supergraph_config,
        )
        .await?;
        Ok(resolved_supergraph_config)
    }

    /// Resolves the subgraph configurations in the supergraph config file such that their file paths
    /// are valid and relative to the supergraph config file (or working directory, if the supergraph
    /// config is piped through stdin
    #[tracing::instrument(skip_all)]
    pub async fn lazily_resolve_subgraphs(
        &self,
        supergraph_config_root: &Utf8PathBuf,
    ) -> Result<
        (
            LazilyResolvedSupergraphConfig,
            BTreeMap<String, ResolveSubgraphError>,
        ),
        ResolveSupergraphConfigError,
    > {
        let unresolved_supergraph_config = UnresolvedSupergraphConfig::builder()
            .and_origin_path(self.state.origin_path.clone())
            .subgraphs(self.state.subgraphs.clone())
            .federation_version_resolver(self.state.federation_version_resolver.clone())
            .build();
        let resolved_supergraph_config = LazilyResolvedSupergraphConfig::resolve(
            supergraph_config_root,
            unresolved_supergraph_config,
        )
        .await;
        Ok(resolved_supergraph_config)
    }
}

/// Object that describes how a default subgraph for composition should be retrieved
pub enum DefaultSubgraphDefinition {
    /// This retrieves default subgraph definitions by prompting the user
    Prompt(Box<dyn Prompt>),
    /// This retrieves default subgraph definitions from CLI args
    Args {
        /// The name of the subgraph
        name: String,
        /// The routing/introspection URL of the subgraph
        url: Url,
        /// The schema path of the subgraph
        schema_path: Option<Utf8PathBuf>,
    },
}

impl DefaultSubgraphDefinition {
    /// Fetches the subgraph name from the definition strategy
    pub fn name(&self) -> Result<String, ResolveSubgraphError> {
        match self {
            DefaultSubgraphDefinition::Prompt(prompt) => prompt.prompt_for_subgraph_name(),
            DefaultSubgraphDefinition::Args { name, .. } => Ok(name.to_string()),
        }
    }

    /// Fetches the subgraph url from the definition strategy
    pub fn url(&self) -> Result<Url, ResolveSubgraphError> {
        match self {
            DefaultSubgraphDefinition::Prompt(prompt) => prompt.prompt_for_subgraph_url(),
            DefaultSubgraphDefinition::Args { url, .. } => Ok(url.clone()),
        }
    }

    /// Fetches the subgraph schema from the definition strategy
    pub fn schema_path(&self) -> Option<Utf8PathBuf> {
        match self {
            DefaultSubgraphDefinition::Prompt(_) => None,
            DefaultSubgraphDefinition::Args { schema_path, .. } => schema_path.clone(),
        }
    }
}

/// A trait for prompting the user for input, primarily for subgraph URL and name. Exists for ease
/// of testing
#[cfg_attr(test, mockall::automock)]
pub trait Prompt {
    /// Prompts user for the subgraph name
    fn prompt_for_subgraph_name(&self) -> Result<String, ResolveSubgraphError>;
    /// Prompts user for the subgraph url
    fn prompt_for_subgraph_url(&self) -> Result<Url, ResolveSubgraphError>;
}

/// Prompts for subgraph URL and name. Implements [Prompt] for ease of testing
#[derive(Default)]
pub struct SubgraphPrompt {}

impl Prompt for SubgraphPrompt {
    fn prompt_for_subgraph_name(&self) -> Result<String, ResolveSubgraphError> {
        if std::io::stderr().is_terminal() {
            let mut input = Input::new().with_prompt("what is the name of this subgraph?");
            if let Some(dirname) = maybe_name_from_dir() {
                input = input.default(dirname);
            }
            let name: String =
                input
                    .interact_text()
                    .map_err(|err| ResolveSubgraphError::InvalidCliInput {
                        input: err.to_string(),
                    })?;

            Ok(name)
        } else {
            let mut cmd = Rover::command();
            cmd.error(
                ClapErrorKind::MissingRequiredArgument,
                "--name <SUBGRAPH_NAME> is required when not attached to a TTY",
            )
            .exit();
        }
    }

    fn prompt_for_subgraph_url(&self) -> Result<Url, ResolveSubgraphError> {
        let url_context = |input| format!("'{}' is not a valid subgraph URL.", &input);
        if std::io::stderr().is_terminal() {
            let input: String = Input::new()
                .with_prompt("what URL is your subgraph running on?")
                .interact_text()
                .map_err(|err| ResolveSubgraphError::InvalidCliInput {
                    input: err.to_string(),
                })?;

            Ok(input
                .parse()
                .with_context(|| url_context(&input))
                .map_err(|err| ResolveSubgraphError::InvalidCliInput {
                    input: err.to_string(),
                })?)
        } else {
            let mut cmd = Rover::command();
            cmd.error(
                ClapErrorKind::MissingRequiredArgument,
                "--url <SUBGRAPH_URL> is required when not attached to a TTY",
            )
            .exit();
        }
    }
}

fn maybe_name_from_dir() -> Option<String> {
    std::env::current_dir()
        .ok()
        .and_then(|x| x.file_name().map(|x| x.to_string_lossy().to_lowercase()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use std::sync::Arc;

    use anyhow::Result;
    use apollo_federation_types::config::{
        FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
    };
    use assert_fs::prelude::{FileTouch, FileWriteStr, PathChild};
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use mockall::predicate;
    use rover_client::RoverClientError;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;
    use tower::{ServiceBuilder, ServiceExt};
    use tower_test::mock::Handle;

    use super::fetch_remote_subgraph::{
        FetchRemoteSubgraphError, FetchRemoteSubgraphFactory, FetchRemoteSubgraphRequest,
        MakeFetchRemoteSubgraphError, RemoteSubgraph,
    };
    use super::fetch_remote_subgraphs::{
        FetchRemoteSubgraphsRequest, MakeFetchRemoteSubgraphsError,
    };
    use super::{DefaultSubgraphDefinition, MockPrompt, SupergraphConfigResolver};
    use crate::composition::supergraph::config::error::ResolveSubgraphError;
    use crate::composition::supergraph::config::full::introspect::{
        MakeResolveIntrospectSubgraphRequest, ResolveIntrospectSubgraphFactory,
    };
    use crate::composition::supergraph::config::full::FullyResolvedSubgraph;
    use crate::composition::supergraph::config::scenario::*;
    use crate::utils::effect::introspect::MockIntrospectSubgraph;
    use crate::utils::effect::read_stdin::MockReadStdin;
    use crate::utils::parsers::FileDescriptorType;

    /// Test showing that federation version is selected from the user-specified fed version
    /// over local supergraph config, remote composition version, or version inferred from
    /// resolved SDLs
    /// For these tests, we only need to test against a remote schema source and a local one.
    /// The sdl schema source was chosen as local, since it's the easiest one to configure
    #[rstest]
    /// Case: both local and remote subgraphs exist with fed 1 SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::One,
            routing_url()
        ))
    )]
    /// Case: only a remote subgraph exists with a fed 1 SDL
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        None
    )]
    /// Case: only a local subgraph exists with a fed 1 SDL
    #[case(
        None,
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::One,
            routing_url()
        ))
    )]
    /// Case: both local and remote subgraphs exist with fed 2 SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::Two
        )),
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::Two,
            routing_url()
        ))
    )]
    /// Case: only a remote subgraph exists with a fed 2 SDL
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::Two
        )),
        None
    )]
    /// Case: only a local subgraph exists with a fed 2 SDL
    #[case(
        None,
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::Two,
            routing_url()
        ))
    )]
    /// Case: both local and remote subgraphs exist with varying fed version SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::Two,
            routing_url()
        ))
    )]
    /// This test further uses #[values] to make sure we have a matrix of tests
    /// All possible combinations result in using the target federation version,
    /// since that is the highest order of precedence
    #[tokio::test]
    async fn test_select_federation_version_from_user_selection(
        #[case] remote_subgraph_scenario: Option<RemoteSubgraphScenario>,
        #[case] sdl_subgraph_scenario: Option<SdlSubgraphScenario>,
        // Dictates whether to load the remote supergraph schema from a the local config or using the --graph_ref flag
        #[values(true, false)] fetch_remote_subgraph_from_config: bool,
        // Dictates whether to load the local supergraph schema from a file or stdin
        #[values(true, false)] load_supergraph_config_from_file: bool,
        // The optional fed version attached to a local supergraph config
        #[values(Some(FederationVersion::LatestFedOne), None)]
        local_supergraph_federation_version: Option<FederationVersion>,
    ) -> Result<()> {
        // user-specified federation version
        let target_federation_version =
            FederationVersion::ExactFedTwo(Version::from_str("2.7.1").unwrap());
        let mut subgraphs = BTreeMap::new();

        let (resolve_introspect_subgraph_service, mut resolve_introspect_subgraph_handle) =
            tower_test::mock::spawn::<(), FullyResolvedSubgraph>();

        let (fetch_remote_subgraphs_service, fetch_remote_subgraphs_handle) =
            tower_test::mock::spawn::<FetchRemoteSubgraphsRequest, BTreeMap<String, SubgraphConfig>>(
            );
        let (fetch_remote_subgraph_service, fetch_remote_subgraph_handle) =
            tower_test::mock::spawn::<FetchRemoteSubgraphRequest, RemoteSubgraph>();

        setup_remote_subgraph_scenario(
            fetch_remote_subgraph_from_config,
            remote_subgraph_scenario.as_ref(),
            &mut subgraphs,
            fetch_remote_subgraphs_handle,
            fetch_remote_subgraph_handle,
        );

        setup_sdl_subgraph_scenario(sdl_subgraph_scenario.as_ref(), &mut subgraphs);

        let mut mock_read_stdin = MockReadStdin::new();

        let local_supergraph_config =
            SupergraphConfig::new(subgraphs, local_supergraph_federation_version);
        let local_supergraph_config_str = serde_yaml::to_string(&local_supergraph_config)?;
        let local_supergraph_config_dir = assert_fs::TempDir::new()?;
        let local_supergraph_config_path =
            Utf8PathBuf::from_path_buf(local_supergraph_config_dir.path().to_path_buf()).unwrap();

        let file_descriptor_type = setup_file_descriptor(
            load_supergraph_config_from_file,
            &local_supergraph_config_dir,
            &local_supergraph_config_str,
            &mut mock_read_stdin,
        )?;

        // init resolver with a target fed version
        let resolver = SupergraphConfigResolver::new(target_federation_version.clone());

        // determine whether to try to load from graph refs
        let graph_ref = remote_subgraph_scenario
            .as_ref()
            .and_then(|remote_subgraph_scenario| {
                if fetch_remote_subgraph_from_config {
                    None
                } else {
                    Some(remote_subgraph_scenario.graph_ref.clone())
                }
            });

        let fetch_remote_subgraphs_factory =
            ServiceBuilder::new()
                .boxed_clone()
                .service_fn(move |_: ()| {
                    let fetch_remote_subgraphs_service = fetch_remote_subgraphs_service.clone();
                    async move {
                        Ok::<_, MakeFetchRemoteSubgraphsError>(
                            ServiceBuilder::new()
                                .map_err(RoverClientError::ServiceReady)
                                .service(fetch_remote_subgraphs_service.into_inner())
                                .boxed_clone(),
                        )
                    }
                });

        // load remote subgraphs
        let resolver = resolver
            .load_remote_subgraphs(fetch_remote_subgraphs_factory, graph_ref.as_ref())
            .await?;

        // load from the file descriptor
        let resolver = resolver
            .load_from_file_descriptor(&mut mock_read_stdin, Some(&file_descriptor_type))?
            .define_default_subgraph_if_empty(DefaultSubgraphDefinition::Prompt(Box::new(
                MockPrompt::default(),
            )))?;

        let fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory = ServiceBuilder::new()
            .boxed_clone()
            .service_fn(move |_: ()| {
                let fetch_remote_subgraph_service = fetch_remote_subgraph_service.clone();
                async move {
                    Ok::<_, MakeFetchRemoteSubgraphError>(
                        ServiceBuilder::new()
                            .map_err(FetchRemoteSubgraphError::Service)
                            .service(fetch_remote_subgraph_service.into_inner())
                            .boxed_clone(),
                    )
                }
            });

        // we never introspect subgraphs in this test, but we still have to account for the effect
        resolve_introspect_subgraph_handle.allow(0);

        let resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory =
            ServiceBuilder::new().boxed_clone().service_fn(
                move |_: MakeResolveIntrospectSubgraphRequest| {
                    let resolve_introspect_subgraph_service =
                        resolve_introspect_subgraph_service.clone();
                    async move {
                        Ok(ServiceBuilder::new()
                            .boxed_clone()
                            .map_err(|err| ResolveSubgraphError::IntrospectionError {
                                subgraph_name: "dont-call-me".to_string(),
                                source: Arc::new(err),
                            })
                            .service(resolve_introspect_subgraph_service.into_inner()))
                    }
                },
            );

        // fully resolve subgraphs into their SDLs
        let (fully_resolved_supergraph_config, _) = resolver
            .fully_resolve_subgraphs(
                resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory,
                &local_supergraph_config_path,
            )
            .await?;

        // validate that the federation version is correct
        assert_that!(fully_resolved_supergraph_config.federation_version())
            .is_equal_to(&target_federation_version);

        Ok(())
    }

    /// Test showing that federation version is selected from the local supergraph config fed version
    /// over remote composition version, or version inferred from resolved SDLs
    /// For these tests, we only need to test against a remote schema source and a local one.
    /// The sdl schema source was chosen as local, since it's the easiest one to configure
    #[rstest]
    /// Case: both local and remote subgraphs exist with fed 1 SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::One,
            routing_url()
        ))
    )]
    /// Case: only a remote subgraph exists with a fed 1 SDL
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        None
    )]
    /// Case: only a local subgraph exists with a fed 1 SDL
    #[case(
        None,
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::One,
            routing_url()
        ))
    )]
    /// Case: both local and remote subgraphs exist with fed 2 SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::Two
        )),
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::Two,
            routing_url()
        ))
    )]
    /// Case: only a remote subgraph exists with a fed 2 SDL
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::Two
        )),
        None
    )]
    /// Case: only a local subgraph exists with a fed 2 SDL
    #[case(
        None,
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::Two,
            routing_url()
        ))
    )]
    /// Case: both local and remote subgraphs exist with varying fed version SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::Two,
            routing_url()
        ))
    )]
    /// This test further uses #[values] to make sure we have a matrix of tests
    /// All possible combinations result in using the remote federation version,
    /// since that is the highest order of precedence in this socpe
    #[trace]
    #[tokio::test]
    async fn test_select_federation_version_from_local_supergraph_config(
        #[case] remote_subgraph_scenario: Option<RemoteSubgraphScenario>,
        #[case] sdl_subgraph_scenario: Option<SdlSubgraphScenario>,
        // Dictates whether to load the remote supergraph schema from a the local config or using the --graph_ref flag
        #[values(true, false)] fetch_remote_subgraph_from_config: bool,
        // Dictates whether to load the local supergraph schema from a file or stdin
        #[values(true, false)] load_supergraph_config_from_file: bool,
    ) -> Result<()> {
        // user-specified federation version (from local supergraph config)
        let local_supergraph_federation_version =
            FederationVersion::ExactFedTwo(Version::from_str("2.7.1").unwrap());

        let mut subgraphs = BTreeMap::new();

        let (resolve_introspect_subgraph_service, mut resolve_introspect_subgraph_handle) =
            tower_test::mock::spawn::<(), FullyResolvedSubgraph>();

        let (fetch_remote_subgraphs_service, fetch_remote_subgraphs_handle) =
            tower_test::mock::spawn::<FetchRemoteSubgraphsRequest, BTreeMap<String, SubgraphConfig>>(
            );
        let (fetch_remote_subgraph_service, fetch_remote_subgraph_handle) =
            tower_test::mock::spawn::<FetchRemoteSubgraphRequest, RemoteSubgraph>();

        setup_remote_subgraph_scenario(
            fetch_remote_subgraph_from_config,
            remote_subgraph_scenario.as_ref(),
            &mut subgraphs,
            fetch_remote_subgraphs_handle,
            fetch_remote_subgraph_handle,
        );

        setup_sdl_subgraph_scenario(sdl_subgraph_scenario.as_ref(), &mut subgraphs);

        let mut mock_read_stdin = MockReadStdin::new();

        let local_supergraph_config =
            SupergraphConfig::new(subgraphs, Some(local_supergraph_federation_version.clone()));
        let local_supergraph_config_str = serde_yaml::to_string(&local_supergraph_config)?;
        let local_supergraph_config_dir = assert_fs::TempDir::new()?;
        let local_supergraph_config_path =
            Utf8PathBuf::from_path_buf(local_supergraph_config_dir.path().to_path_buf()).unwrap();

        let file_descriptor_type = setup_file_descriptor(
            load_supergraph_config_from_file,
            &local_supergraph_config_dir,
            &local_supergraph_config_str,
            &mut mock_read_stdin,
        )?;

        // init resolver with no target fed version
        let resolver = SupergraphConfigResolver::default();

        // determine whether to try to load from graph refs
        let graph_ref = remote_subgraph_scenario
            .as_ref()
            .and_then(|remote_subgraph_scenario| {
                if fetch_remote_subgraph_from_config {
                    None
                } else {
                    Some(remote_subgraph_scenario.graph_ref.clone())
                }
            });

        let fetch_remote_subgraphs_factory =
            ServiceBuilder::new()
                .boxed_clone()
                .service_fn(move |_: ()| {
                    let fetch_remote_subgraphs_service = fetch_remote_subgraphs_service.clone();
                    async move {
                        Ok::<_, MakeFetchRemoteSubgraphsError>(
                            ServiceBuilder::new()
                                .map_err(RoverClientError::ServiceReady)
                                .service(fetch_remote_subgraphs_service.into_inner())
                                .boxed_clone(),
                        )
                    }
                });

        // load remote subgraphs
        let resolver = resolver
            .load_remote_subgraphs(fetch_remote_subgraphs_factory, graph_ref.as_ref())
            .await?;

        // load from the file descriptor
        let resolver = resolver
            .load_from_file_descriptor(&mut mock_read_stdin, Some(&file_descriptor_type))?
            .define_default_subgraph_if_empty(DefaultSubgraphDefinition::Prompt(Box::new(
                MockPrompt::default(),
            )))?;

        let fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory = ServiceBuilder::new()
            .boxed_clone()
            .service_fn(move |_: ()| {
                let fetch_remote_subgraph_service = fetch_remote_subgraph_service.clone();
                async move {
                    Ok::<_, MakeFetchRemoteSubgraphError>(
                        ServiceBuilder::new()
                            .map_err(FetchRemoteSubgraphError::Service)
                            .service(fetch_remote_subgraph_service.into_inner())
                            .boxed_clone(),
                    )
                }
            });

        // we never introspect subgraphs in this test, but we still have to account for the effect
        resolve_introspect_subgraph_handle.allow(0);

        let resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory =
            ServiceBuilder::new().boxed_clone().service_fn(
                move |_: MakeResolveIntrospectSubgraphRequest| {
                    let resolve_introspect_subgraph_service =
                        resolve_introspect_subgraph_service.clone();
                    async move {
                        Ok(ServiceBuilder::new()
                            .boxed_clone()
                            .map_err(|err| ResolveSubgraphError::IntrospectionError {
                                subgraph_name: "dont-call-me".to_string(),
                                source: Arc::new(err),
                            })
                            .service(resolve_introspect_subgraph_service.into_inner()))
                    }
                },
            );

        // fully resolve subgraphs into their SDLs
        let (fully_resolved_supergraph_config, _) = resolver
            .fully_resolve_subgraphs(
                resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory,
                &local_supergraph_config_path,
            )
            .await?;

        // validate that the federation version is correct
        assert_that!(fully_resolved_supergraph_config.federation_version())
            .is_equal_to(&local_supergraph_federation_version);

        Ok(())
    }

    /// Test showing that federation version is selected from the local supergraph config fed version
    /// over remote composition version, or version inferred from resolved SDLs
    /// For these tests, we only need to test against a remote schema source and a local one.
    /// The sdl schema source was chosen as local, since it's the easiest one to configure
    #[rstest]
    /// Case: both local and remote subgraphs exist with fed 1 SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::One,
            routing_url()
        ))
    )]
    /// Case: only a remote subgraph exists with a fed 1 SDL
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        None
    )]
    /// Case: only a local subgraph exists with a fed 1 SDL
    #[case(
        None,
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::One,
            routing_url()
        ))
    )]
    /// Case: both local and remote subgraphs exist with fed 2 SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::Two
        )),
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::Two,
            routing_url()
        ))
    )]
    /// Case: only a remote subgraph exists with a fed 2 SDL
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::Two
        )),
        None
    )]
    /// Case: only a local subgraph exists with a fed 2 SDL
    #[case(
        None,
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::Two,
            routing_url()
        ))
    )]
    /// Case: both local and remote subgraphs exist with varying fed version SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        Some(sdl_subgraph_scenario(
            sdl(),
            subgraph_name(),
            SubgraphFederationVersion::Two,
            routing_url()
        ))
    )]
    /// This test further uses #[values] to make sure we have a matrix of tests
    /// All possible combinations result in using the remote federation version,
    /// since that is the highest order of precedence in this socpe
    #[trace]
    #[tokio::test]
    async fn test_select_federation_version_defaults_to_fed_two(
        #[case] remote_subgraph_scenario: Option<RemoteSubgraphScenario>,
        #[case] sdl_subgraph_scenario: Option<SdlSubgraphScenario>,
        // Dictates whether to load the remote supergraph schema from a the local config or using the --graph_ref flag
        #[values(true, false)] fetch_remote_subgraph_from_config: bool,
        // Dictates whether to load the local supergraph schema from a file or stdin
        #[values(true, false)] load_supergraph_config_from_file: bool,
    ) -> Result<()> {
        let mut subgraphs = BTreeMap::new();

        let (resolve_introspect_subgraph_service, mut resolve_introspect_subgraph_handle) =
            tower_test::mock::spawn::<(), FullyResolvedSubgraph>();

        let (fetch_remote_subgraphs_service, fetch_remote_subgraphs_handle) =
            tower_test::mock::spawn::<FetchRemoteSubgraphsRequest, BTreeMap<String, SubgraphConfig>>(
            );
        let (fetch_remote_subgraph_service, fetch_remote_subgraph_handle) =
            tower_test::mock::spawn::<FetchRemoteSubgraphRequest, RemoteSubgraph>();

        setup_remote_subgraph_scenario(
            fetch_remote_subgraph_from_config,
            remote_subgraph_scenario.as_ref(),
            &mut subgraphs,
            fetch_remote_subgraphs_handle,
            fetch_remote_subgraph_handle,
        );

        setup_sdl_subgraph_scenario(sdl_subgraph_scenario.as_ref(), &mut subgraphs);

        let mut mock_read_stdin = MockReadStdin::new();

        let local_supergraph_config = SupergraphConfig::new(subgraphs, None);
        let local_supergraph_config_str = serde_yaml::to_string(&local_supergraph_config)?;
        let local_supergraph_config_dir = assert_fs::TempDir::new()?;
        let local_supergraph_config_path =
            Utf8PathBuf::from_path_buf(local_supergraph_config_dir.path().to_path_buf()).unwrap();

        let file_descriptor_type = setup_file_descriptor(
            load_supergraph_config_from_file,
            &local_supergraph_config_dir,
            &local_supergraph_config_str,
            &mut mock_read_stdin,
        )?;

        // we never introspect subgraphs in this test, but we still have to account for the effect
        let mut mock_introspect_subgraph = MockIntrospectSubgraph::new();
        mock_introspect_subgraph
            .expect_introspect_subgraph()
            .times(0);

        // init resolver with no target fed version
        let resolver = SupergraphConfigResolver::default();

        // determine whether to try to load from graph refs
        let graph_ref = remote_subgraph_scenario
            .as_ref()
            .and_then(|remote_subgraph_scenario| {
                if fetch_remote_subgraph_from_config {
                    None
                } else {
                    Some(remote_subgraph_scenario.graph_ref.clone())
                }
            });

        let fetch_remote_subgraphs_factory =
            ServiceBuilder::new()
                .boxed_clone()
                .service_fn(move |_: ()| {
                    let fetch_remote_subgraphs_service = fetch_remote_subgraphs_service.clone();
                    async move {
                        Ok::<_, MakeFetchRemoteSubgraphsError>(
                            ServiceBuilder::new()
                                .map_err(RoverClientError::ServiceReady)
                                .service(fetch_remote_subgraphs_service.into_inner())
                                .boxed_clone(),
                        )
                    }
                });

        // load remote subgraphs
        let resolver = resolver
            .load_remote_subgraphs(fetch_remote_subgraphs_factory, graph_ref.as_ref())
            .await?;

        // load from the file descriptor
        let resolver = resolver
            .load_from_file_descriptor(&mut mock_read_stdin, Some(&file_descriptor_type))?
            .define_default_subgraph_if_empty(DefaultSubgraphDefinition::Prompt(Box::new(
                MockPrompt::default(),
            )))?;

        let fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory = ServiceBuilder::new()
            .boxed_clone()
            .service_fn(move |_: ()| {
                let fetch_remote_subgraph_service = fetch_remote_subgraph_service.clone();
                async move {
                    Ok::<_, MakeFetchRemoteSubgraphError>(
                        ServiceBuilder::new()
                            .map_err(FetchRemoteSubgraphError::Service)
                            .service(fetch_remote_subgraph_service.into_inner())
                            .boxed_clone(),
                    )
                }
            });

        resolve_introspect_subgraph_handle.allow(0);

        let resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory =
            ServiceBuilder::new().boxed_clone().service_fn(
                move |_: MakeResolveIntrospectSubgraphRequest| {
                    let resolve_introspect_subgraph_service =
                        resolve_introspect_subgraph_service.clone();
                    async move {
                        Ok(ServiceBuilder::new()
                            .boxed_clone()
                            .map_err(|err| ResolveSubgraphError::IntrospectionError {
                                subgraph_name: "dont-call-me".to_string(),
                                source: Arc::new(err),
                            })
                            .service(resolve_introspect_subgraph_service.into_inner()))
                    }
                },
            );

        // fully resolve subgraphs into their SDLs
        let (fully_resolved_supergraph_config, _) = resolver
            .fully_resolve_subgraphs(
                resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory,
                &local_supergraph_config_path,
            )
            .await?;

        // validate that the federation version is correct
        assert_that!(fully_resolved_supergraph_config.federation_version())
            .is_equal_to(&FederationVersion::LatestFedTwo);

        Ok(())
    }

    fn setup_sdl_subgraph_scenario(
        sdl_subgraph_scenario: Option<&SdlSubgraphScenario>,
        local_subgraphs: &mut BTreeMap<String, SubgraphConfig>,
    ) {
        // If the sdl subgraph scenario exists, add a SubgraphConfig for it to the supergraph config
        if let Some(sdl_subgraph_scenario) = sdl_subgraph_scenario {
            let schema_source = SchemaSource::Sdl {
                sdl: sdl_subgraph_scenario.sdl.to_string(),
            };
            let subgraph_config = SubgraphConfig {
                routing_url: Some(routing_url()),
                schema: schema_source,
            };
            local_subgraphs.insert("sdl-subgraph".to_string(), subgraph_config);
        }
    }

    fn setup_remote_subgraph_scenario(
        fetch_remote_subgraph_from_config: bool,
        remote_subgraph_scenario: Option<&RemoteSubgraphScenario>,
        local_subgraphs: &mut BTreeMap<String, SubgraphConfig>,
        mut fetch_remote_subgraphs_handle: Handle<
            FetchRemoteSubgraphsRequest,
            BTreeMap<String, SubgraphConfig>,
        >,
        mut fetch_remote_subgraph_handle: Handle<FetchRemoteSubgraphRequest, RemoteSubgraph>,
    ) {
        if let Some(remote_subgraph_scenario) = remote_subgraph_scenario {
            let schema_source = SchemaSource::Subgraph {
                graphref: remote_subgraph_scenario.graph_ref.to_string(),
                subgraph: remote_subgraph_scenario.subgraph_name.to_string(),
            };
            let subgraph_config = SubgraphConfig {
                routing_url: Some(remote_subgraph_scenario.routing_url.clone()),
                schema: schema_source,
            };
            // If the remote subgraph scenario exists, add a SubgraphConfig for it to the supergraph config
            if fetch_remote_subgraph_from_config {
                local_subgraphs.insert("remote-subgraph".to_string(), subgraph_config);
                fetch_remote_subgraphs_handle.allow(0);
            }
            // Otherwise, fetch it by --graph_ref
            else {
                fetch_remote_subgraphs_handle.allow(1);
                tokio::spawn({
                    let remote_subgraph_scenario = remote_subgraph_scenario.clone();
                    async move {
                        let (req, send_response) =
                            fetch_remote_subgraphs_handle.next_request().await.unwrap();
                        assert_that!(req).is_equal_to(FetchRemoteSubgraphsRequest::new(
                            remote_subgraph_scenario.graph_ref.clone(),
                        ));
                        let subgraph_name = remote_subgraph_scenario.subgraph_name.to_string();
                        send_response.send_response(BTreeMap::from_iter([(
                            subgraph_name.to_string(),
                            subgraph_config.clone(),
                        )]));
                    }
                });
            }

            // we always fetch the SDLs from remote
            fetch_remote_subgraph_handle.allow(1);
            tokio::spawn({
                let remote_subgraph_scenario = remote_subgraph_scenario.clone();
                async move {
                    let (req, send_response) =
                        fetch_remote_subgraph_handle.next_request().await.unwrap();
                    assert_that!(req).is_equal_to(
                        FetchRemoteSubgraphRequest::builder()
                            .graph_ref(remote_subgraph_scenario.graph_ref.clone())
                            .subgraph_name(remote_subgraph_scenario.subgraph_name.clone())
                            .build(),
                    );
                    let subgraph_name = remote_subgraph_scenario.subgraph_name.to_string();
                    let routing_url = remote_subgraph_scenario.routing_url.to_string();
                    let sdl = remote_subgraph_scenario.sdl.to_string();
                    send_response.send_response(
                        RemoteSubgraph::builder()
                            .name(subgraph_name.to_string())
                            .routing_url(routing_url.to_string())
                            .schema(sdl.to_string())
                            .build(),
                    )
                }
            });
        } else {
            // if no remote subgraph schemas exist, don't expect them to fetched
            fetch_remote_subgraphs_handle.allow(0);
            fetch_remote_subgraph_handle.allow(0);
        }
    }

    fn setup_file_descriptor(
        load_supergraph_config_from_file: bool,
        local_supergraph_config_dir: &TempDir,
        local_supergraph_config_str: &str,
        mock_read_stdin: &mut MockReadStdin,
    ) -> Result<FileDescriptorType> {
        let file_descriptor_type = if load_supergraph_config_from_file {
            // if we should be loading the supergraph config from a file, set up the temp files to do so
            let local_supergraph_config_file = local_supergraph_config_dir.child("supergraph.yaml");
            local_supergraph_config_file.touch()?;
            local_supergraph_config_file.write_str(local_supergraph_config_str)?;
            let path =
                Utf8PathBuf::from_path_buf(local_supergraph_config_file.path().to_path_buf())
                    .unwrap();
            mock_read_stdin.expect_read_stdin().times(0);
            FileDescriptorType::File(path)
        } else {
            // otherwise, mock read_stdin to provide the string back
            mock_read_stdin
                .expect_read_stdin()
                .times(1)
                .with(predicate::eq("supergraph config"))
                .returning({
                    let local_supergraph_config_str = local_supergraph_config_str.to_string();
                    move |_| Ok(local_supergraph_config_str.to_string())
                });
            FileDescriptorType::Stdin
        };
        Ok(file_descriptor_type)
    }

    #[rstest]
    #[case::env_var_set(vec![("MY_ENV_VAR", Some("foo.bar.com"))], "http://foo.bar.com:5000/graphql")]
    #[case::default_value_used(vec![], "http://host.docker.internal:5000/graphql")]
    #[tokio::test]
    async fn test_expansion_works_inside_supergraph_yaml(
        #[case] kvs: Vec<(&str, Option<&str>)>,
        #[case] expected: &str,
    ) {
        let supergraph_config = r#"federation_version: =2.9.3
subgraphs:
  products:
    routing_url: http://${env.MY_ENV_VAR:-host.docker.internal}:5000/graphql
    schema:
      subgraph_url: http://localhost:4001
  users:
    routing_url: http://localhost:4002
    schema:
      subgraph_url: http://localhost:4002"#;
        let local_supergraph_config_dir = TempDir::new().expect("Couldn't create temp dir.");

        let mut mock_stdin = MockReadStdin::new();

        let file_descriptor_type = setup_file_descriptor(
            true,
            &local_supergraph_config_dir,
            supergraph_config,
            &mut mock_stdin,
        )
        .expect("Couldn't setup file descriptor.");

        let (fetch_remote_subgraphs_service, _) = tower_test::mock::spawn::<
            FetchRemoteSubgraphsRequest,
            BTreeMap<String, SubgraphConfig>,
        >();

        let fetch_remote_subgraphs_factory =
            ServiceBuilder::new()
                .boxed_clone()
                .service_fn(move |_: ()| {
                    let fetch_remote_subgraphs_service = fetch_remote_subgraphs_service.clone();
                    async move {
                        Ok::<_, MakeFetchRemoteSubgraphsError>(
                            ServiceBuilder::new()
                                .map_err(RoverClientError::ServiceReady)
                                .service(fetch_remote_subgraphs_service.into_inner())
                                .boxed_clone(),
                        )
                    }
                });

        temp_env::async_with_vars(kvs, async {
            let resolver = SupergraphConfigResolver::default()
                .load_remote_subgraphs(fetch_remote_subgraphs_factory, None)
                .await
                .expect("Couldn't load remote subgraphs.")
                .load_from_file_descriptor(&mut mock_stdin, Some(&file_descriptor_type))
                .expect("Couldn't load local subgraphs.")
                .skip_default_subgraph();

            assert_that!(
                resolver
                    .state
                    .subgraphs
                    .get("products")
                    .unwrap()
                    .routing_url
            )
            .is_some()
            .is_equal_to(String::from(expected));
        })
        .await;
    }
}
