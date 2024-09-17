use std::collections::BTreeMap;

use apollo_federation_types::config::{FederationVersion, SubgraphConfig, SupergraphConfig};
use camino::Utf8PathBuf;
use futures::{
    stream::{self, StreamExt},
    TryFutureExt,
};
use itertools::Itertools;

use crate::utils::effect::{
    fetch_remote_subgraph::FetchRemoteSubgraph, introspect::IntrospectSubgraph,
};

use self::subgraph::{
    FullyResolvedSubgraph, LazilyResolvedSubgraph, ResolveSubgraphError, UnresolvedSubgraph,
};

pub mod subgraph;

pub struct UnresolvedSupergraphConfig {
    subgraphs: BTreeMap<String, UnresolvedSubgraph>,
    federation_version: FederationVersion,
}

impl UnresolvedSupergraphConfig {
    pub fn new(supergraph_config: SupergraphConfig) -> UnresolvedSupergraphConfig {
        let federation_version = supergraph_config
            .get_federation_version()
            .unwrap_or(FederationVersion::LatestFedTwo);
        let subgraphs = supergraph_config
            .into_iter()
            .map(|(name, subgraph)| (name.to_string(), UnresolvedSubgraph::new(name, subgraph)))
            .collect();
        UnresolvedSupergraphConfig {
            subgraphs,
            federation_version,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSupergraphConfig<ResolvedSubgraph: Into<SubgraphConfig>> {
    subgraphs: BTreeMap<String, ResolvedSubgraph>,
    federation_version: FederationVersion,
}

impl<T: Into<SubgraphConfig>> From<ResolvedSupergraphConfig<T>> for SupergraphConfig {
    fn from(value: ResolvedSupergraphConfig<T>) -> Self {
        let subgraphs = value
            .subgraphs
            .into_iter()
            .map(|(name, subgraph)| (name, subgraph.into()))
            .collect();
        SupergraphConfig::new(subgraphs, Some(value.federation_version))
    }
}

impl ResolvedSupergraphConfig<FullyResolvedSubgraph> {
    pub async fn resolve(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        supergraph_config_root: &Utf8PathBuf,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> Result<ResolvedSupergraphConfig<FullyResolvedSubgraph>, Vec<ResolveSubgraphError>> {
        let subgraphs = stream::iter(unresolved_supergraph_config.subgraphs.into_iter().map(
            |(name, unresolved_subgraph)| {
                FullyResolvedSubgraph::resolve(
                    introspect_subgraph_impl,
                    fetch_remote_subgraph_impl,
                    supergraph_config_root,
                    unresolved_subgraph,
                )
                .map_ok(|result| (name, result))
            },
        ))
        .buffer_unordered(50)
        .collect::<Vec<Result<(String, FullyResolvedSubgraph), ResolveSubgraphError>>>()
        .await;
        let (subgraphs, errors): (
            Vec<(String, FullyResolvedSubgraph)>,
            Vec<ResolveSubgraphError>,
        ) = subgraphs.into_iter().partition_result();
        if errors.is_empty() {
            Ok(ResolvedSupergraphConfig {
                subgraphs: BTreeMap::from_iter(subgraphs),
                federation_version: unresolved_supergraph_config.federation_version,
            })
        } else {
            Err(errors)
        }
    }
}

impl ResolvedSupergraphConfig<LazilyResolvedSubgraph> {
    pub async fn resolve(
        supergraph_config_root: &Utf8PathBuf,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> Result<ResolvedSupergraphConfig<LazilyResolvedSubgraph>, Vec<ResolveSubgraphError>> {
        let subgraphs = stream::iter(unresolved_supergraph_config.subgraphs.into_iter().map(
            |(name, unresolved_subgraph)| async {
                let result =
                    LazilyResolvedSubgraph::resolve(supergraph_config_root, unresolved_subgraph)?;
                Ok((name, result))
            },
        ))
        .buffer_unordered(50)
        .collect::<Vec<Result<(String, LazilyResolvedSubgraph), ResolveSubgraphError>>>()
        .await;
        let (subgraphs, errors): (
            Vec<(String, LazilyResolvedSubgraph)>,
            Vec<ResolveSubgraphError>,
        ) = subgraphs.into_iter().partition_result();
        if errors.is_empty() {
            Ok(ResolvedSupergraphConfig {
                subgraphs: BTreeMap::from_iter(subgraphs),
                federation_version: unresolved_supergraph_config.federation_version,
            })
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {

    use std::{collections::BTreeMap, str::FromStr};

    use anyhow::Result;
    use apollo_federation_types::config::FederationVersion;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use mockall::predicate;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use crate::utils::effect::{
        fetch_remote_subgraph::{MockFetchRemoteSubgraph, RemoteSubgraph},
        introspect::MockIntrospectSubgraph,
    };

    use super::{
        subgraph::{scenerio::*, FullyResolvedSubgraph, LazilyResolvedSubgraph},
        ResolvedSupergraphConfig, UnresolvedSupergraphConfig,
    };

    #[fixture]
    fn supergraph_config_root_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[rstest]
    #[tokio::test]
    async fn test_fully_resolve_subgraphs(
        supergraph_config_root_dir: TempDir,
        sdl_subgraph_scenario: SdlSubgraphScenario,
        remote_subgraph_scenario: RemoteSubgraphScenario,
        introspect_subgraph_scenario: IntrospectSubgraphScenario,
        file_subgraph_scenario: FileSubgraphScenario,
    ) -> Result<()> {
        file_subgraph_scenario.write_schema_file(supergraph_config_root_dir.path())?;
        let mut unresolved_subgraphs = BTreeMap::new();
        unresolved_subgraphs.insert(
            "sdl_subgraph".to_string(),
            sdl_subgraph_scenario.unresolved_subgraph,
        );
        unresolved_subgraphs.insert(
            "remote_subgraph".to_string(),
            remote_subgraph_scenario.unresolved_subgraph,
        );
        unresolved_subgraphs.insert(
            "introspect_subgraph".to_string(),
            introspect_subgraph_scenario.unresolved_subgraph,
        );
        unresolved_subgraphs.insert(
            "file_subgraph".to_string(),
            file_subgraph_scenario.unresolved_subgraph,
        );

        let unresolved_supergraph_config = UnresolvedSupergraphConfig {
            subgraphs: unresolved_subgraphs,
            federation_version: FederationVersion::LatestFedTwo,
        };

        let RemoteSubgraphScenario {
            sdl: remote_subgraph_sdl,
            graph_ref: remote_subgraph_graph_ref,
            subgraph_name: remote_subgraph_subgraph_name,
            routing_url: remote_subgraph_routing_url,
            ..
        } = remote_subgraph_scenario;

        let mut mock_fetch_remote_subgraph = MockFetchRemoteSubgraph::new();
        mock_fetch_remote_subgraph
            .expect_fetch_remote_subgraph()
            .times(1)
            .with(
                predicate::eq(remote_subgraph_graph_ref.clone()),
                predicate::eq(remote_subgraph_subgraph_name.to_string()),
            )
            .returning({
                {
                    let remote_subgraph_sdl = remote_subgraph_sdl.to_string();
                    move |_, name| {
                        Ok(RemoteSubgraph::builder()
                            .name(name.to_string())
                            .routing_url(remote_subgraph_routing_url.to_string())
                            .schema(remote_subgraph_sdl.to_string())
                            .build())
                    }
                }
            });

        let IntrospectSubgraphScenario {
            sdl: introspect_subgraph_sdl,
            routing_url: introspect_subgraph_routing_url,
            introspection_headers: introspect_subgraph_introspection_headers,
            ..
        } = introspect_subgraph_scenario;

        let mut mock_introspect_subgraph = MockIntrospectSubgraph::new();
        mock_introspect_subgraph
            .expect_introspect_subgraph()
            .times(1)
            .with(
                predicate::eq(url::Url::from_str(&introspect_subgraph_routing_url)?),
                predicate::eq(introspect_subgraph_introspection_headers),
            )
            .returning({
                let introspect_subgraph_sdl = introspect_subgraph_sdl.to_string();
                move |_, _| Ok(introspect_subgraph_sdl.to_string())
            });

        let result = <ResolvedSupergraphConfig<FullyResolvedSubgraph>>::resolve(
            &mock_introspect_subgraph,
            &mock_fetch_remote_subgraph,
            &Utf8PathBuf::from_path_buf(supergraph_config_root_dir.path().to_path_buf()).unwrap(),
            unresolved_supergraph_config,
        )
        .await;

        mock_fetch_remote_subgraph.checkpoint();
        mock_introspect_subgraph.checkpoint();

        assert_that!(result).is_ok();

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn lazily_resolve_subgraphs(
        supergraph_config_root_dir: TempDir,
        sdl_subgraph_scenario: SdlSubgraphScenario,
        remote_subgraph_scenario: RemoteSubgraphScenario,
        introspect_subgraph_scenario: IntrospectSubgraphScenario,
        file_subgraph_scenario: FileSubgraphScenario,
    ) -> Result<()> {
        file_subgraph_scenario.write_schema_file(supergraph_config_root_dir.path())?;
        let mut unresolved_subgraphs = BTreeMap::new();
        unresolved_subgraphs.insert(
            "sdl_subgraph".to_string(),
            sdl_subgraph_scenario.unresolved_subgraph,
        );
        unresolved_subgraphs.insert(
            "remote_subgraph".to_string(),
            remote_subgraph_scenario.unresolved_subgraph,
        );
        unresolved_subgraphs.insert(
            "introspect_subgraph".to_string(),
            introspect_subgraph_scenario.unresolved_subgraph,
        );
        unresolved_subgraphs.insert(
            "file_subgraph".to_string(),
            file_subgraph_scenario.unresolved_subgraph,
        );

        let unresolved_supergraph_config = UnresolvedSupergraphConfig {
            subgraphs: unresolved_subgraphs,
            federation_version: FederationVersion::LatestFedTwo,
        };

        let result = <ResolvedSupergraphConfig<LazilyResolvedSubgraph>>::resolve(
            &Utf8PathBuf::from_path_buf(supergraph_config_root_dir.path().to_path_buf()).unwrap(),
            unresolved_supergraph_config,
        )
        .await;
        assert_that!(result).is_ok();

        Ok(())
    }
}
