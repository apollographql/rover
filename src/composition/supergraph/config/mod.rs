use std::collections::BTreeMap;

use apollo_federation_types::config::{
    ConfigError, FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
};
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use thiserror::Error;

use crate::{
    utils::{
        effect::{
            fetch_remote_subgraph::FetchRemoteSubgraph,
            fetch_remote_subgraphs::FetchRemoteSubgraphs, introspect::IntrospectSubgraph,
            read_stdin::ReadStdin,
        },
        parsers::FileDescriptorType,
    },
    RoverError,
};

use self::{
    resolve::{
        subgraph::ResolveSubgraphError, FullyResolvedSupergraphConfig,
        LazilyResolvedSupergraphConfig, UnresolvedSupergraphConfig,
    },
    state::{LoadRemoteSubgraphs, ResolveSubgraphs},
};

pub mod resolve;

mod state {
    use std::collections::BTreeMap;

    use apollo_federation_types::config::{FederationVersion, SubgraphConfig};
    use camino::Utf8PathBuf;

    pub struct LoadRemoteSubgraphs {
        pub federation_version: Option<FederationVersion>,
    }
    pub struct LoadSupergraphConfig {
        pub federation_version: Option<FederationVersion>,
        pub subgraphs: BTreeMap<String, SubgraphConfig>,
    }
    pub struct ResolveSubgraphs {
        pub origin_path: Option<Utf8PathBuf>,
        pub federation_version: Option<FederationVersion>,
        pub subgraphs: BTreeMap<String, SubgraphConfig>,
    }
}

use state::LoadSupergraphConfig;

pub struct SupergraphConfigResolver<State> {
    state: State,
}

impl SupergraphConfigResolver<LoadRemoteSubgraphs> {
    pub fn new(
        federation_version: Option<FederationVersion>,
    ) -> SupergraphConfigResolver<LoadRemoteSubgraphs> {
        SupergraphConfigResolver {
            state: LoadRemoteSubgraphs { federation_version },
        }
    }
}

impl Default for SupergraphConfigResolver<LoadRemoteSubgraphs> {
    fn default() -> Self {
        Self::new(None)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadRemoteSubgraphsError {
    #[error(transparent)]
    FetchRemoteSubgraphsError(Box<dyn std::error::Error + Send + Sync>),
}

impl SupergraphConfigResolver<LoadRemoteSubgraphs> {
    pub async fn load_remote_subgraphs(
        self,
        fetch_remote_subgraphs_impl: &impl FetchRemoteSubgraphs,
        graph_ref: Option<&GraphRef>,
    ) -> Result<SupergraphConfigResolver<LoadSupergraphConfig>, LoadRemoteSubgraphsError> {
        if let Some(graph_ref) = graph_ref {
            let remote_subgraphs = fetch_remote_subgraphs_impl
                .fetch_remote_subgraphs(graph_ref)
                .await
                .map_err(|err| {
                    LoadRemoteSubgraphsError::FetchRemoteSubgraphsError(Box::new(err))
                })?;
            Ok(SupergraphConfigResolver {
                state: LoadSupergraphConfig {
                    federation_version: self.state.federation_version,
                    subgraphs: remote_subgraphs,
                },
            })
        } else {
            Ok(SupergraphConfigResolver {
                state: LoadSupergraphConfig {
                    federation_version: self.state.federation_version,
                    subgraphs: BTreeMap::default(),
                },
            })
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadSupergraphConfigError {
    #[error("Failed to parse the supergraph config. Error: {0}")]
    SupergraphConfig(ConfigError),
    #[error("Failed to read file descriptor. Error: {0}")]
    ReadFileDescriptor(RoverError),
}

impl SupergraphConfigResolver<LoadSupergraphConfig> {
    pub fn load_from_file_descriptor(
        self,
        read_stdin_impl: &mut impl ReadStdin,
        file_descriptor_type: Option<&FileDescriptorType>,
    ) -> Result<SupergraphConfigResolver<ResolveSubgraphs>, LoadSupergraphConfigError> {
        if let Some(file_descriptor_type) = file_descriptor_type {
            let supergraph_config = file_descriptor_type
                .read_file_descriptor("supergraph config", read_stdin_impl)
                .map_err(LoadSupergraphConfigError::ReadFileDescriptor)
                .and_then(|contents| {
                    SupergraphConfig::new_from_yaml(&contents)
                        .map_err(LoadSupergraphConfigError::SupergraphConfig)
                })?;
            let origin_path = match file_descriptor_type {
                FileDescriptorType::File(file) => Some(file.clone()),
                FileDescriptorType::Stdin => None,
            };
            let federation_version = self
                .state
                .federation_version
                .or_else(|| supergraph_config.get_federation_version());
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
                state: ResolveSubgraphs {
                    origin_path,
                    federation_version,
                    subgraphs: merged_subgraphs,
                },
            })
        } else {
            Ok(SupergraphConfigResolver {
                state: ResolveSubgraphs {
                    origin_path: None,
                    federation_version: self.state.federation_version,
                    subgraphs: self.state.subgraphs,
                },
            })
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ResolveSupergraphConfigError {
    #[error("No source found for supergraph config")]
    NoSource,
    #[error("Unable to resolve subgraphs")]
    ResolveSubgraphs(Vec<ResolveSubgraphError>),
    #[error(
        "The 'federation_version' specified ({}) is invalid. The following subgraphs contain '@link' directives, which are only valid in Federation 2: {}",
        specified_federation_version,
        subgraph_names.join(", ")
    )]
    FederationVersionMismatch {
        specified_federation_version: FederationVersion,
        subgraph_names: Vec<String>,
    },
}

impl SupergraphConfigResolver<ResolveSubgraphs> {
    pub async fn fully_resolve_subgraphs(
        self,
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        supergraph_config_root: &Utf8PathBuf,
    ) -> Result<FullyResolvedSupergraphConfig, ResolveSupergraphConfigError> {
        if !self.state.subgraphs.is_empty() {
            let unresolved_supergraph_config = UnresolvedSupergraphConfig::builder()
                .subgraphs(self.state.subgraphs)
                .and_federation_version(self.state.federation_version)
                .build();
            let resolved_supergraph_config = FullyResolvedSupergraphConfig::resolve(
                introspect_subgraph_impl,
                fetch_remote_subgraph_impl,
                supergraph_config_root,
                unresolved_supergraph_config,
            )
            .await?;
            Ok(resolved_supergraph_config)
        } else {
            Err(ResolveSupergraphConfigError::NoSource)
        }
    }

    pub async fn lazily_resolve_subgraphs(
        self,
        supergraph_config_root: &Utf8PathBuf,
    ) -> Result<LazilyResolvedSupergraphConfig, ResolveSupergraphConfigError> {
        if !self.state.subgraphs.is_empty() {
            let unresolved_supergraph_config = UnresolvedSupergraphConfig::builder()
                .subgraphs(self.state.subgraphs)
                .and_federation_version(self.state.federation_version)
                .build();
            let resolved_supergraph_config = LazilyResolvedSupergraphConfig::resolve(
                supergraph_config_root,
                unresolved_supergraph_config,
            )
            .await
            .map_err(ResolveSupergraphConfigError::ResolveSubgraphs)?;
            Ok(resolved_supergraph_config)
        } else {
            Err(ResolveSupergraphConfigError::NoSource)
        }
    }
}

#[derive(Error, Debug)]
#[error("Invalid schema source: {:?}", .schema_source)]
pub struct InvalidSchemaSource {
    schema_source: SchemaSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullyResolvedSubgraphs {
    subgraphs: BTreeMap<String, String>,
}

impl FullyResolvedSubgraphs {
    pub fn new(subgraphs: BTreeMap<String, String>) -> FullyResolvedSubgraphs {
        FullyResolvedSubgraphs { subgraphs }
    }

    pub fn upsert_subgraph(&mut self, name: String, schema: String) {
        self.subgraphs.insert(name, schema);
    }

    pub fn remove_subgraph(&mut self, name: &str) {
        self.subgraphs.remove(name);
    }
}

impl TryFrom<SupergraphConfig> for FullyResolvedSubgraphs {
    type Error = Vec<InvalidSchemaSource>;
    fn try_from(value: SupergraphConfig) -> Result<Self, Self::Error> {
        let mut errors = Vec::new();
        let mut subgraph_sdls = BTreeMap::new();
        for (name, subgraph_config) in value.into_iter() {
            if let SchemaSource::Sdl { sdl } = subgraph_config.schema {
                subgraph_sdls.insert(name, sdl);
            } else {
                errors.push(InvalidSchemaSource {
                    schema_source: subgraph_config.schema,
                });
            }
        }
        if errors.is_empty() {
            Ok(FullyResolvedSubgraphs {
                subgraphs: subgraph_sdls,
            })
        } else {
            Err(errors)
        }
    }
}

impl From<FullyResolvedSubgraphs> for SupergraphConfig {
    fn from(value: FullyResolvedSubgraphs) -> Self {
        let subgraphs = BTreeMap::from_iter(value.subgraphs.into_iter().map(|(name, sdl)| {
            (
                name,
                SubgraphConfig {
                    routing_url: None,
                    schema: SchemaSource::Sdl { sdl },
                },
            )
        }));
        SupergraphConfig::new(subgraphs, None)
    }
}

// #[derive(thiserror::Error, Debug)]
// pub enum WriteSupergraphConfigError {
//     #[error("Unable to serialized the supergraph config")]
//     Serialization(#[from] serde_yaml::Error),
//     #[error("Unable to write file")]
//     Fs(RoverStdError),
// }

// #[derive(Clone, Debug, Getters)]
// pub struct FinalSupergraphConfig {
//     origin_path: Option<Utf8PathBuf>,
//     #[getter(skip)]
//     federation_version: FederationVersion,
//     // map of subgraph name to SDL
//     #[getter(skip)]
//     subgraphs: BTreeMap<String, String>,
// }

// impl FinalSupergraphConfig {
//     #[cfg(test)]
//     pub fn new(
//         origin_path: Option<Utf8PathBuf>,
//         federation_version: FederationVersion,
//         subgraphs: BTreeMap<String, String>,
//     ) -> FinalSupergraphConfig {
//         Ok(FinalSupergraphConfig {
//             federation_version,
//             subgraphs,
//             origin_path,
//         })
//     }
// }

// impl From<&FinalSupergraphConfig> for SupergraphConfig {
//     fn from(value: &FinalSupergraphConfig) -> Self {
//         let subgraphs = BTreeMap::from_iter(value.subgraphs.iter().map(|(name, sdl)| {
//             (
//                 name.to_string(),
//                 SubgraphConfig {
//                     routing_url: None,
//                     schema: SchemaSource::Sdl {
//                         sdl: sdl.to_string(),
//                     },
//                 },
//             )
//         }));
//         SupergraphConfig::new(subgraphs, Some(value.federation_version.clone()))
//     }
// }

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, str::FromStr};

    use anyhow::Result;
    use apollo_federation_types::config::{
        FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
    };
    use assert_fs::{
        prelude::{FileTouch, FileWriteStr, PathChild},
        TempDir,
    };
    use camino::Utf8PathBuf;
    use mockall::predicate;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use crate::utils::{
        effect::{
            fetch_remote_subgraph::{MockFetchRemoteSubgraph, RemoteSubgraph},
            fetch_remote_subgraphs::MockFetchRemoteSubgraphs,
            introspect::MockIntrospectSubgraph,
            read_stdin::MockReadStdin,
        },
        parsers::FileDescriptorType,
    };

    use super::{resolve::subgraph::scenario::*, SupergraphConfigResolver};

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
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::One))
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
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::One))
    )]
    /// Case: both local and remote subgraphs exist with fed 2 SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::Two
        )),
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::Two))
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
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::Two))
    )]
    /// Case: both local and remote subgraphs exist with varying fed version SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::Two))
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

        let mut mock_fetch_remote_subgraphs = MockFetchRemoteSubgraphs::new();
        let mut mock_fetch_remote_subgraph = MockFetchRemoteSubgraph::new();

        setup_remote_subgraph_scenario(
            fetch_remote_subgraph_from_config,
            remote_subgraph_scenario.as_ref(),
            &mut subgraphs,
            &mut mock_fetch_remote_subgraphs,
            &mut mock_fetch_remote_subgraph,
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

        // we never introspect subgraphs in this test, but we still have to account for the effect
        let mut mock_introspect_subgraph = MockIntrospectSubgraph::new();
        mock_introspect_subgraph
            .expect_introspect_subgraph()
            .times(0);

        // init resolver with a target fed version
        let resolver = SupergraphConfigResolver::new(Some(target_federation_version.clone()));

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

        // load remote subgraphs
        let resolver = resolver
            .load_remote_subgraphs(&mock_fetch_remote_subgraphs, graph_ref.as_ref())
            .await?;

        // load from the file descriptor
        let resolver = resolver
            .load_from_file_descriptor(&mut mock_read_stdin, Some(&file_descriptor_type))?;

        // validate that the correct effect has been invoked
        mock_fetch_remote_subgraphs.checkpoint();

        // fully resolve subgraphs into their SDLs
        let fully_resolved_supergraph_config = resolver
            .fully_resolve_subgraphs(
                &mock_introspect_subgraph,
                &mock_fetch_remote_subgraph,
                &local_supergraph_config_path,
            )
            .await?;

        // validate that the correct effects have been invoked
        mock_introspect_subgraph.checkpoint();
        mock_fetch_remote_subgraph.checkpoint();

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
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::One))
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
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::One))
    )]
    /// Case: both local and remote subgraphs exist with fed 2 SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::Two
        )),
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::Two))
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
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::Two))
    )]
    /// Case: both local and remote subgraphs exist with varying fed version SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::Two))
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

        let mut mock_fetch_remote_subgraphs = MockFetchRemoteSubgraphs::new();
        let mut mock_fetch_remote_subgraph = MockFetchRemoteSubgraph::new();

        setup_remote_subgraph_scenario(
            fetch_remote_subgraph_from_config,
            remote_subgraph_scenario.as_ref(),
            &mut subgraphs,
            &mut mock_fetch_remote_subgraphs,
            &mut mock_fetch_remote_subgraph,
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

        // we never introspect subgraphs in this test, but we still have to account for the effect
        let mut mock_introspect_subgraph = MockIntrospectSubgraph::new();
        mock_introspect_subgraph
            .expect_introspect_subgraph()
            .times(0);

        // init resolver with no target fed version
        let resolver = SupergraphConfigResolver::new(None);

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

        // load remote subgraphs
        let resolver = resolver
            .load_remote_subgraphs(&mock_fetch_remote_subgraphs, graph_ref.as_ref())
            .await?;

        // load from the file descriptor
        let resolver = resolver
            .load_from_file_descriptor(&mut mock_read_stdin, Some(&file_descriptor_type))?;

        // validate that the correct effect has been invoked
        mock_fetch_remote_subgraphs.checkpoint();

        // fully resolve subgraphs into their SDLs
        let fully_resolved_supergraph_config = resolver
            .fully_resolve_subgraphs(
                &mock_introspect_subgraph,
                &mock_fetch_remote_subgraph,
                &local_supergraph_config_path,
            )
            .await?;

        // validate that the correct effects have been invoked
        mock_introspect_subgraph.checkpoint();
        mock_fetch_remote_subgraph.checkpoint();

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
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::One))
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
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::One))
    )]
    /// Case: both local and remote subgraphs exist with fed 2 SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::Two
        )),
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::Two))
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
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::Two))
    )]
    /// Case: both local and remote subgraphs exist with varying fed version SDLs
    #[case(
        Some(remote_subgraph_scenario(
            sdl(),
            subgraph_name(),
            routing_url(),
            SubgraphFederationVersion::One
        )),
        Some(sdl_subgraph_scenario(sdl(), subgraph_name(), SubgraphFederationVersion::Two))
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

        let mut mock_fetch_remote_subgraphs = MockFetchRemoteSubgraphs::new();
        let mut mock_fetch_remote_subgraph = MockFetchRemoteSubgraph::new();

        setup_remote_subgraph_scenario(
            fetch_remote_subgraph_from_config,
            remote_subgraph_scenario.as_ref(),
            &mut subgraphs,
            &mut mock_fetch_remote_subgraphs,
            &mut mock_fetch_remote_subgraph,
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
        let resolver = SupergraphConfigResolver::new(None);

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

        // load remote subgraphs
        let resolver = resolver
            .load_remote_subgraphs(&mock_fetch_remote_subgraphs, graph_ref.as_ref())
            .await?;

        // load from the file descriptor
        let resolver = resolver
            .load_from_file_descriptor(&mut mock_read_stdin, Some(&file_descriptor_type))?;

        // validate that the correct effect has been invoked
        mock_fetch_remote_subgraphs.checkpoint();

        // fully resolve subgraphs into their SDLs
        let fully_resolved_supergraph_config = resolver
            .fully_resolve_subgraphs(
                &mock_introspect_subgraph,
                &mock_fetch_remote_subgraph,
                &local_supergraph_config_path,
            )
            .await?;

        // validate that the correct effects have been invoked
        mock_introspect_subgraph.checkpoint();
        mock_fetch_remote_subgraph.checkpoint();

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
                routing_url: None,
                schema: schema_source,
            };
            local_subgraphs.insert("sdl-subgraph".to_string(), subgraph_config);
        }
    }

    fn setup_remote_subgraph_scenario(
        fetch_remote_subgraph_from_config: bool,
        remote_subgraph_scenario: Option<&RemoteSubgraphScenario>,
        local_subgraphs: &mut BTreeMap<String, SubgraphConfig>,
        mock_fetch_remote_subgraphs: &mut MockFetchRemoteSubgraphs,
        mock_fetch_remote_subgraph: &mut MockFetchRemoteSubgraph,
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
                mock_fetch_remote_subgraphs
                    .expect_fetch_remote_subgraphs()
                    .times(0);
            }
            // Otherwise, fetch it by --graph_ref
            else {
                mock_fetch_remote_subgraphs
                    .expect_fetch_remote_subgraphs()
                    .times(1)
                    .with(predicate::eq(remote_subgraph_scenario.graph_ref.clone()))
                    .returning({
                        let subgraph_name = remote_subgraph_scenario.subgraph_name.to_string();
                        move |_| {
                            Ok(BTreeMap::from_iter([(
                                subgraph_name.to_string(),
                                subgraph_config.clone(),
                            )]))
                        }
                    });
            }

            // we always fetch the SDLs from remote
            mock_fetch_remote_subgraph
                .expect_fetch_remote_subgraph()
                .times(1)
                .with(
                    predicate::eq(remote_subgraph_scenario.graph_ref.clone()),
                    predicate::eq(remote_subgraph_scenario.subgraph_name.clone()),
                )
                .returning({
                    let subgraph_name = remote_subgraph_scenario.subgraph_name.to_string();
                    let routing_url = remote_subgraph_scenario.routing_url.to_string();
                    let sdl = remote_subgraph_scenario.sdl.to_string();
                    move |_, _| {
                        Ok(RemoteSubgraph::builder()
                            .name(subgraph_name.to_string())
                            .routing_url(routing_url.to_string())
                            .schema(sdl.to_string())
                            .build())
                    }
                });
        } else {
            // if no remote subgraph schemas exist, don't expect them to fetched
            mock_fetch_remote_subgraphs
                .expect_fetch_remote_subgraphs()
                .times(0);
            mock_fetch_remote_subgraph
                .expect_fetch_remote_subgraph()
                .times(0);
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
}
