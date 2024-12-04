//! Provides utilities to resolve subgraphs, fully or lazily

use std::{path::PathBuf, str::FromStr};

use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use apollo_parser::{cst, Parser};
use buildstructor::{buildstructor, Builder};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use rover_std::Fs;

use crate::utils::effect::{
    fetch_remote_subgraph::FetchRemoteSubgraph, introspect::IntrospectSubgraph,
};

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use anyhow::Result;
    use apollo_federation_types::config::SchemaSource;
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
        scenario::{
            file_subgraph_scenario, introspect_subgraph_scenario, remote_subgraph_scenario,
            sdl_subgraph_scenario, FileSubgraphScenario, IntrospectSubgraphScenario,
            RemoteSubgraphScenario, SdlSubgraphScenario,
        },
        FullyResolvedSubgraph, LazilyResolvedSubgraph, ResolveSubgraphError,
    };

    #[fixture]
    fn supergraph_config_root_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[rstest]
    #[tokio::test]
    async fn test_fully_resolve_sdl_subgraph_success(
        supergraph_config_root_dir: TempDir,
        sdl_subgraph_scenario: SdlSubgraphScenario,
    ) -> Result<()> {
        let SdlSubgraphScenario {
            sdl,
            unresolved_subgraph,
            subgraph_federation_version,
        } = sdl_subgraph_scenario;
        // No fetch remote subgraph or introspect subgraph calls should be made
        let mut mock_fetch_remote_subgraph = MockFetchRemoteSubgraph::new();
        mock_fetch_remote_subgraph
            .expect_fetch_remote_subgraph()
            .times(0);
        let mut mock_introspect_subgraph = MockIntrospectSubgraph::new();
        mock_introspect_subgraph
            .expect_introspect_subgraph()
            .times(0);

        // WHEN we lazily resolve an unresolved subgraph against the supergraph config root
        let result = FullyResolvedSubgraph::resolve(
            &mock_introspect_subgraph,
            &mock_fetch_remote_subgraph,
            Some(&Utf8PathBuf::try_from(
                supergraph_config_root_dir.path().to_path_buf(),
            )?),
            unresolved_subgraph,
        )
        .await;

        // THEN we assert the mock implementations were called correctly
        mock_fetch_remote_subgraph.checkpoint();
        mock_introspect_subgraph.checkpoint();

        // THEN we have a SchemaSource::File resolved to the canonicalized file path
        assert_that!(result)
            .is_ok()
            .is_equal_to(FullyResolvedSubgraph {
                routing_url: None,
                schema: sdl,
                is_fed_two: subgraph_federation_version.is_fed_two(),
            });
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fully_resolve_remote_subgraph_success(
        supergraph_config_root_dir: TempDir,
        remote_subgraph_scenario: RemoteSubgraphScenario,
    ) -> Result<()> {
        let RemoteSubgraphScenario {
            sdl,
            graph_ref,
            unresolved_subgraph,
            subgraph_name,
            routing_url,
            subgraph_federation_version,
        } = remote_subgraph_scenario;
        let mut mock_fetch_remote_subgraph = MockFetchRemoteSubgraph::new();
        mock_fetch_remote_subgraph
            .expect_fetch_remote_subgraph()
            .times(1)
            .with(
                predicate::eq(graph_ref.clone()),
                predicate::eq(subgraph_name.to_string()),
            )
            .returning({
                let routing_url = routing_url.to_string();
                {
                    let sdl = sdl.to_string();
                    move |_, name| {
                        Ok(RemoteSubgraph::builder()
                            .name(name.to_string())
                            .routing_url(routing_url.to_string())
                            .schema(sdl.to_string())
                            .build())
                    }
                }
            });

        // GIVEN we have a IntrospectSubgraph implementation that does not get called
        let mut mock_introspect_subgraph = MockIntrospectSubgraph::new();
        mock_introspect_subgraph
            .expect_introspect_subgraph()
            .times(0);

        // WHEN we lazily resolve an unresolved subgraph against the supergraph config root
        let result = FullyResolvedSubgraph::resolve(
            &mock_introspect_subgraph,
            &mock_fetch_remote_subgraph,
            Some(&Utf8PathBuf::try_from(
                supergraph_config_root_dir.path().to_path_buf(),
            )?),
            unresolved_subgraph,
        )
        .await;

        // THEN we assert the mock implementations were called correctly
        mock_introspect_subgraph.checkpoint();
        mock_fetch_remote_subgraph.checkpoint();

        // THEN we have a SchemaSource::File resolved to the canonicalized file path
        assert_that!(result)
            .is_ok()
            .is_equal_to(FullyResolvedSubgraph {
                routing_url: Some(routing_url),
                schema: sdl.to_string(),
                is_fed_two: subgraph_federation_version.is_fed_two(),
            });
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fully_resolve_introspection_subgraph_success(
        supergraph_config_root_dir: TempDir,
        introspect_subgraph_scenario: IntrospectSubgraphScenario,
    ) -> Result<()> {
        let IntrospectSubgraphScenario {
            sdl,
            routing_url,
            introspection_headers,
            unresolved_subgraph,
            subgraph_federation_version,
        } = introspect_subgraph_scenario;
        let mut mock_introspect_subgraph = MockIntrospectSubgraph::new();
        mock_introspect_subgraph
            .expect_introspect_subgraph()
            .times(1)
            .with(
                predicate::eq(url::Url::from_str(&routing_url)?),
                predicate::eq(introspection_headers),
            )
            .returning({
                let sdl = sdl.to_string();
                move |_, _| Ok(sdl.to_string())
            });

        let mut mock_fetch_remote_subgraph = MockFetchRemoteSubgraph::new();
        mock_fetch_remote_subgraph
            .expect_fetch_remote_subgraph()
            .times(0);

        // WHEN we fully resolve an unresolved subgraph against the supergraph config root
        let result = FullyResolvedSubgraph::resolve(
            &mock_introspect_subgraph,
            &mock_fetch_remote_subgraph,
            Some(&Utf8PathBuf::try_from(
                supergraph_config_root_dir.path().to_path_buf(),
            )?),
            unresolved_subgraph,
        )
        .await;

        // THEN we assert the mock implementations were called correctly
        mock_introspect_subgraph.checkpoint();
        mock_fetch_remote_subgraph.checkpoint();

        // THEN we have a SchemaSource::File resolved to the canonicalized file path
        assert_that!(result)
            .is_ok()
            .is_equal_to(FullyResolvedSubgraph {
                routing_url: Some(routing_url),
                schema: sdl.to_string(),
                is_fed_two: subgraph_federation_version.is_fed_two(),
            });
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fully_resolve_file_subgraph_success(
        supergraph_config_root_dir: TempDir,
        file_subgraph_scenario: FileSubgraphScenario,
    ) -> Result<()> {
        // GIVEN there is a file in the supergraph config root dir
        file_subgraph_scenario.write_schema_file(supergraph_config_root_dir.path())?;
        let FileSubgraphScenario {
            sdl,
            routing_url,
            unresolved_subgraph,
            subgraph_federation_version,
            ..
        } = file_subgraph_scenario;

        // GIVEN we have a IntrospectSubgraph implementation
        let mut mock_introspect_subgraph = MockIntrospectSubgraph::new();
        mock_introspect_subgraph
            .expect_introspect_subgraph()
            .times(0);

        // GIVEN we have a FetchRemoteSubgraph implementation
        let mut mock_fetch_remote_subgraph = MockFetchRemoteSubgraph::new();
        mock_fetch_remote_subgraph
            .expect_fetch_remote_subgraph()
            .times(0);

        // WHEN we lazily resolve an unresolved subgraph against the supergraph config root
        let result = FullyResolvedSubgraph::resolve(
            &mock_introspect_subgraph,
            &mock_fetch_remote_subgraph,
            Some(&Utf8PathBuf::try_from(
                supergraph_config_root_dir.path().to_path_buf(),
            )?),
            unresolved_subgraph,
        )
        .await;

        // THEN we assert the mock implementations were called correctly
        mock_introspect_subgraph.checkpoint();
        mock_fetch_remote_subgraph.checkpoint();

        // THEN we have a SchemaSource::File resolved to the canonicalized file path
        assert_that!(result)
            .is_ok()
            .is_equal_to(FullyResolvedSubgraph {
                routing_url: Some(routing_url),
                schema: sdl.to_string(),
                is_fed_two: subgraph_federation_version.is_fed_two(),
            });
        Ok(())
    }

    #[rstest]
    fn test_lazily_resolve_file_subgraph_success(
        supergraph_config_root_dir: TempDir,
        file_subgraph_scenario: FileSubgraphScenario,
    ) -> Result<()> {
        // GIVEN there is a file in the supergraph config root dir
        file_subgraph_scenario.write_schema_file(supergraph_config_root_dir.path())?;

        let FileSubgraphScenario {
            routing_url,
            schema_file_path,
            unresolved_subgraph,
            ..
        } = file_subgraph_scenario;

        let result = LazilyResolvedSubgraph::resolve(
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
            unresolved_subgraph,
        );

        assert_that!(result)
            .is_ok()
            .is_equal_to(LazilyResolvedSubgraph {
                routing_url: Some(routing_url),
                schema: SchemaSource::File {
                    file: Utf8PathBuf::from_path_buf(
                        supergraph_config_root_dir.path().join(schema_file_path),
                    )
                    .unwrap()
                    .canonicalize_utf8()?,
                },
            });
        Ok(())
    }

    #[rstest]
    fn test_lazily_resolve_file_subgraph_failure(
        supergraph_config_root_dir: TempDir,
        file_subgraph_scenario: FileSubgraphScenario,
    ) -> Result<()> {
        // GIVEN there is a schema file outside of the supergraph config root dir
        let other_root_dir = TempDir::new()?;
        file_subgraph_scenario.write_schema_file(other_root_dir.path())?;

        let FileSubgraphScenario {
            unresolved_subgraph,
            schema_file_path,
            subgraph_name,
            ..
        } = file_subgraph_scenario;

        let result = LazilyResolvedSubgraph::resolve(
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
            unresolved_subgraph,
        );

        // THEN we should receive an error that the path was unable to be resolved
        let subject = assert_that!(result).is_err().subject;
        if let ResolveSubgraphError::FileNotFound {
            subgraph_name: actual_subgraph_name,
            supergraph_config_path: supergraph_yaml_path,
            path,
            ..
        } = subject
        {
            assert_that!(actual_subgraph_name).is_equal_to(&subgraph_name);
            assert_that!(supergraph_yaml_path).is_equal_to(
                &Utf8PathBuf::from_path_buf(supergraph_config_root_dir.path().to_path_buf())
                    .unwrap(),
            );
            assert_that!(path).is_equal_to(&schema_file_path.as_std_path().to_path_buf());
        } else {
            panic!("error was not ResolveSubgraphError::FileNotFound");
        };
        Ok(())
    }
}
