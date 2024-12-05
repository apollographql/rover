//! Provides the SupergraphConfigResolver, which is required to run composition or its subgraph/config watchers

#![warn(missing_docs)]

pub mod error;
pub mod federation;
pub mod full;
pub mod lazy;
pub mod resolver;
#[cfg(test)]
pub mod scenario;
pub mod unresolved;

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

    use super::{resolver::SupergraphConfigResolver, scenario::*};

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
                Some(&local_supergraph_config_path),
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
                Some(&local_supergraph_config_path),
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
                Some(&local_supergraph_config_path),
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
