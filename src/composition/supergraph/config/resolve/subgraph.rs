use std::{path::PathBuf, str::FromStr};

use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use rover_std::Fs;

use crate::utils::effect::{
    fetch_remote_subgraph::FetchRemoteSubgraph, introspect::IntrospectSubgraph,
};

#[derive(thiserror::Error, Debug)]
pub enum ResolveSubgraphError {
    #[error("Could not find schema file ({path}) relative to ({supergraph_yaml_path}) for subgraph `{subgraph_name}`")]
    FileNotFound {
        subgraph_name: String,
        supergraph_yaml_path: Utf8PathBuf,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Fs(Box<dyn std::error::Error>),
    #[error("Failed to introspect the subgraph {name}.")]
    IntrospectionError {
        name: String,
        error: Box<dyn std::error::Error>,
    },
    #[error("Invalid graph ref: {graph_ref}")]
    InvalidGraphRef {
        graph_ref: String,
        error: Box<dyn std::error::Error>,
    },
    #[error("Failed to fetch the sdl for subgraph `{name}` from remote")]
    FetchRemoteSdlError {
        name: String,
        error: Box<dyn std::error::Error>,
    },
    #[error(
        "The subgraph `{name}` with graph ref `{graph_ref}` does not have an assigned routing url"
    )]
    MissingRoutingUrl { name: String, graph_ref: GraphRef },
}

pub struct UnresolvedSubgraph {
    name: String,
    schema: SchemaSource,
    routing_url: Option<String>,
}

impl UnresolvedSubgraph {
    pub fn new(name: String, config: SubgraphConfig) -> UnresolvedSubgraph {
        UnresolvedSubgraph {
            name,
            schema: config.schema,
            routing_url: config.routing_url,
        }
    }
    pub fn resolve_file_path(
        &self,
        root: &Utf8PathBuf,
        path: &Utf8PathBuf,
    ) -> Result<Utf8PathBuf, ResolveSubgraphError> {
        let joined_path = root.join(path);
        let canonical_filename = joined_path.canonicalize_utf8();
        match canonical_filename {
            Ok(canonical_filename) => Ok(canonical_filename),
            Err(err) => Err(ResolveSubgraphError::FileNotFound {
                subgraph_name: self.name.to_string(),
                supergraph_yaml_path: root.clone(),
                path: path.as_std_path().to_path_buf(),
                source: err,
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FullyResolvedSubgraph {
    routing_url: Option<String>,
    schema: String,
}

impl FullyResolvedSubgraph {
    pub async fn resolve(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        supergraph_config_root: &Utf8PathBuf,
        unresolved_subgraph: UnresolvedSubgraph,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        match &unresolved_subgraph.schema {
            SchemaSource::File { file } => {
                let file = unresolved_subgraph.resolve_file_path(supergraph_config_root, file)?;
                let schema =
                    Fs::read_file(&file).map_err(|err| ResolveSubgraphError::Fs(Box::new(err)))?;
                Ok(FullyResolvedSubgraph {
                    routing_url: unresolved_subgraph.routing_url.clone(),
                    schema,
                })
            }
            SchemaSource::SubgraphIntrospection {
                subgraph_url,
                introspection_headers,
            } => {
                let schema = introspect_subgraph_impl
                    .introspect_subgraph(
                        subgraph_url.clone(),
                        introspection_headers.clone().unwrap_or_default(),
                    )
                    .await
                    .map_err(|err| ResolveSubgraphError::IntrospectionError {
                        name: unresolved_subgraph.name.to_string(),
                        error: Box::new(err),
                    })?;
                let routing_url = unresolved_subgraph
                    .routing_url
                    .clone()
                    .or_else(|| Some(subgraph_url.to_string()));
                Ok(FullyResolvedSubgraph {
                    routing_url,
                    schema,
                })
            }
            SchemaSource::Subgraph {
                graphref: graph_ref,
                subgraph,
            } => {
                let graph_ref = GraphRef::from_str(graph_ref).map_err(|err| {
                    ResolveSubgraphError::InvalidGraphRef {
                        graph_ref: graph_ref.clone(),
                        error: Box::new(err),
                    }
                })?;
                let remote_subgraph = fetch_remote_subgraph_impl
                    .fetch_remote_subgraph(graph_ref, subgraph.to_string())
                    .await
                    .map_err(|err| ResolveSubgraphError::FetchRemoteSdlError {
                        name: subgraph.to_string(),
                        error: Box::new(err),
                    })?;
                Ok(FullyResolvedSubgraph {
                    routing_url: unresolved_subgraph
                        .routing_url
                        .clone()
                        .or(Some(remote_subgraph.routing_url().to_string())),
                    schema: remote_subgraph.schema().clone(),
                })
            }
            SchemaSource::Sdl { sdl } => Ok(FullyResolvedSubgraph {
                routing_url: None,
                schema: sdl.to_string(),
            }),
        }
    }
}

impl From<FullyResolvedSubgraph> for SubgraphConfig {
    fn from(value: FullyResolvedSubgraph) -> Self {
        SubgraphConfig {
            routing_url: value.routing_url,
            schema: SchemaSource::Sdl { sdl: value.schema },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LazilyResolvedSubgraph {
    routing_url: Option<String>,
    schema: SchemaSource,
}

impl LazilyResolvedSubgraph {
    pub fn resolve(
        supergraph_config_root: &Utf8PathBuf,
        unresolved_subgraph: UnresolvedSubgraph,
    ) -> Result<LazilyResolvedSubgraph, ResolveSubgraphError> {
        match &unresolved_subgraph.schema {
            SchemaSource::File { file } => {
                let file = unresolved_subgraph.resolve_file_path(supergraph_config_root, file)?;
                Ok(LazilyResolvedSubgraph {
                    routing_url: unresolved_subgraph.routing_url.clone(),
                    schema: SchemaSource::File { file },
                })
            }
            _ => Ok(LazilyResolvedSubgraph {
                routing_url: unresolved_subgraph.routing_url.clone(),
                schema: unresolved_subgraph.schema.clone(),
            }),
        }
    }
}

impl From<LazilyResolvedSubgraph> for SubgraphConfig {
    fn from(value: LazilyResolvedSubgraph) -> Self {
        SubgraphConfig {
            routing_url: value.routing_url,
            schema: value.schema,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, io::Write, path::PathBuf, str::FromStr};

    use anyhow::Result;
    use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
    use camino::Utf8PathBuf;
    use mockall::predicate;
    use rover_client::shared::GraphRef;
    use rstest::rstest;
    use speculoos::prelude::*;
    use tempfile::tempdir;

    use crate::utils::effect::{
        fetch_remote_subgraph::{MockFetchRemoteSubgraph, RemoteSubgraph},
        introspect::MockIntrospectSubgraph,
    };

    use super::{
        FullyResolvedSubgraph, LazilyResolvedSubgraph, ResolveSubgraphError, UnresolvedSubgraph,
    };

    #[rstest]
    #[tokio::test]
    async fn test_fully_resolve_sdl_subgraph_success() -> Result<()> {
        // GIVEN there is a supergraph config root dir
        let supergraph_config_root_dir = tempdir()?;

        // GIVEN there is a schema that is available for introspection
        let schema = "type Query { test: String! }";

        let schema_source = SchemaSource::Sdl {
            sdl: schema.to_string(),
        };

        // GIVEN we have an unresolved subgraph
        let unresolved_subgraph = UnresolvedSubgraph::new(
            "subgraph_name".to_string(),
            SubgraphConfig {
                routing_url: None,
                schema: schema_source.clone(),
            },
        );

        // GIVEN we have a FetchRemoteSubgraph implementation that does not get called
        let mut mock_fetch_remote_subgraph_impl = MockFetchRemoteSubgraph::new();
        mock_fetch_remote_subgraph_impl
            .expect_fetch_remote_subgraph()
            .times(0);

        // GIVEN we have a IntrospectSubgraph implementation that does not get called
        let mut mock_introspect_subgraph_impl = MockIntrospectSubgraph::new();
        mock_introspect_subgraph_impl
            .expect_introspect_subgraph()
            .times(0);

        // WHEN we lazily resolve an unresolved subgraph against the supergraph config root
        let result = FullyResolvedSubgraph::resolve(
            &mock_introspect_subgraph_impl,
            &mock_fetch_remote_subgraph_impl,
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
            unresolved_subgraph,
        )
        .await;

        // THEN we assert the mock implementations were called correctly
        mock_introspect_subgraph_impl.checkpoint();
        mock_fetch_remote_subgraph_impl.checkpoint();

        // THEN we have a SchemaSource::File resolved to the canonicalized file path
        assert_that!(result)
            .is_ok()
            .is_equal_to(FullyResolvedSubgraph {
                routing_url: None,
                schema: schema.to_string(),
            });
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fully_resolve_remote_subgraph_success() -> Result<()> {
        // GIVEN there is a supergraph config root dir
        let supergraph_config_root_dir = tempdir()?;

        // GIVEN there is a schema that is available for introspection
        let schema = "type Query { test: String! }";

        // GIVEN a graph exists in studio
        let graph_ref = GraphRef::from_str("my-graph@my-variant")?;
        // AND it has a name
        let subgraph_name = "my-subgraph-name";

        let schema_source = SchemaSource::Subgraph {
            graphref: graph_ref.clone().to_string(),
            subgraph: subgraph_name.to_string(),
        };

        // GIVEN we have a FetchRemoteSubgraph implementation that responds to the given graph ref and subgraph name
        let routing_url = "routing_url".to_string();
        let mut mock_fetch_remote_subgraph_impl = MockFetchRemoteSubgraph::new();
        mock_fetch_remote_subgraph_impl
            .expect_fetch_remote_subgraph()
            .times(1)
            .with(
                predicate::eq(graph_ref.clone()),
                predicate::eq(subgraph_name.to_string()),
            )
            .returning({
                let routing_url = routing_url.to_string();
                move |_, name| {
                    Ok(RemoteSubgraph::builder()
                        .name(name.to_string())
                        .routing_url(routing_url.to_string())
                        .schema(schema.to_string())
                        .build())
                }
            });

        // GIVEN we have an unresolved subgraph
        let unresolved_subgraph = UnresolvedSubgraph::new(
            "subgraph_name".to_string(),
            SubgraphConfig {
                routing_url: Some(routing_url.clone()),
                schema: schema_source.clone(),
            },
        );

        // GIVEN we have a IntrospectSubgraph implementation that does not get called
        let mut mock_introspect_subgraph_impl = MockIntrospectSubgraph::new();
        mock_introspect_subgraph_impl
            .expect_introspect_subgraph()
            .times(0);

        // WHEN we lazily resolve an unresolved subgraph against the supergraph config root
        let result = FullyResolvedSubgraph::resolve(
            &mock_introspect_subgraph_impl,
            &mock_fetch_remote_subgraph_impl,
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
            unresolved_subgraph,
        )
        .await;

        // THEN we assert the mock implementations were called correctly
        mock_introspect_subgraph_impl.checkpoint();
        mock_fetch_remote_subgraph_impl.checkpoint();

        // THEN we have a SchemaSource::File resolved to the canonicalized file path
        assert_that!(result)
            .is_ok()
            .is_equal_to(FullyResolvedSubgraph {
                routing_url: Some(routing_url),
                schema: schema.to_string(),
            });
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fully_resolve_introspection_subgraph_success() -> Result<()> {
        // GIVEN there is a supergraph config root dir
        let supergraph_config_root_dir = tempdir()?;

        // GIVEN there is a schema that is available for introspection
        let schema = "type Query { test: String! }";

        // GIVEN we have a IntrospectSubgraph implementation
        // AND it has a routing_url
        // AND it has headers needed for introspection
        let endpoint = url::Url::from_str("http://example.com")?;
        let introspection_headers = HashMap::from_iter([(
            "x-introspection-key".to_string(),
            "x-introspection-header".to_string(),
        )]);
        let schema_source = SchemaSource::SubgraphIntrospection {
            subgraph_url: endpoint.clone(),
            introspection_headers: Some(introspection_headers.clone()),
        };
        let routing_url = Some("routing_url".to_string());
        let mut mock_introspect_subgraph_impl = MockIntrospectSubgraph::new();
        mock_introspect_subgraph_impl
            .expect_introspect_subgraph()
            .times(1)
            .with(
                predicate::eq(endpoint),
                predicate::eq(introspection_headers),
            )
            .returning(|_, _| Ok(schema.to_string()));

        // GIVEN we have an unresolved subgraph from the
        let unresolved_subgraph = UnresolvedSubgraph::new(
            "subgraph_name".to_string(),
            SubgraphConfig {
                routing_url: routing_url.clone(),
                schema: schema_source.clone(),
            },
        );

        // GIVEN we have a FetchRemoteSubgraph implementation
        let mut mock_fetch_remote_subgraph_impl = MockFetchRemoteSubgraph::new();
        mock_fetch_remote_subgraph_impl
            .expect_fetch_remote_subgraph()
            .times(0);

        // WHEN we lazily resolve an unresolved subgraph against the supergraph config root
        let result = FullyResolvedSubgraph::resolve(
            &mock_introspect_subgraph_impl,
            &mock_fetch_remote_subgraph_impl,
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
            unresolved_subgraph,
        )
        .await;

        // THEN we assert the mock implementations were called correctly
        mock_introspect_subgraph_impl.checkpoint();
        mock_fetch_remote_subgraph_impl.checkpoint();

        // THEN we have a SchemaSource::File resolved to the canonicalized file path
        assert_that!(result)
            .is_ok()
            .is_equal_to(FullyResolvedSubgraph {
                routing_url,
                schema: schema.to_string(),
            });
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fully_resolve_file_subgraph_success() -> Result<()> {
        // GIVEN there is a supergraph config root dir
        let supergraph_config_root_dir = tempdir()?;
        // GIVEN there is a file in the supergraph config root dir
        let full_schema_path =
            Utf8PathBuf::try_from(supergraph_config_root_dir.path().join("schema.graphql"))?;
        // GIVEN this file has a schema
        let schema = "type Query { test: String! }";
        let mut file = fs::File::create(full_schema_path.as_std_path())?;
        file.write_all(schema.as_bytes())?;

        // GIVEN the Schema source is a relative path from the supergraph config root
        let schema_source = SchemaSource::File {
            file: Utf8PathBuf::from("./schema.graphql"),
        };
        let routing_url = Some("routing_url".to_string());

        // GIVEN we have an unresolved subgraph from the
        let unresolved_subgraph = UnresolvedSubgraph::new(
            "subgraph_name".to_string(),
            SubgraphConfig {
                routing_url: routing_url.clone(),
                schema: schema_source.clone(),
            },
        );

        // GIVEN we have a IntrospectSubgraph implementation
        let mut mock_introspect_subgraph_impl = MockIntrospectSubgraph::new();
        mock_introspect_subgraph_impl
            .expect_introspect_subgraph()
            .times(0);

        // GIVEN we have a FetchRemoteSubgraph implementation
        let mut mock_fetch_remote_subgraph_impl = MockFetchRemoteSubgraph::new();
        mock_fetch_remote_subgraph_impl
            .expect_fetch_remote_subgraph()
            .times(0);

        // WHEN we lazily resolve an unresolved subgraph against the supergraph config root
        let result = FullyResolvedSubgraph::resolve(
            &mock_introspect_subgraph_impl,
            &mock_fetch_remote_subgraph_impl,
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
            unresolved_subgraph,
        )
        .await;

        // THEN we assert the mock implementations were called correctly
        mock_introspect_subgraph_impl.checkpoint();
        mock_fetch_remote_subgraph_impl.checkpoint();

        // THEN we have a SchemaSource::File resolved to the canonicalized file path
        assert_that!(result)
            .is_ok()
            .is_equal_to(FullyResolvedSubgraph {
                routing_url,
                schema: schema.to_string(),
            });
        Ok(())
    }

    #[test]
    fn test_lazily_resolve_file_subgraph_success() -> Result<()> {
        // GIVEN there is a supergraph config root dir
        let supergraph_config_root_dir = tempdir()?;
        // GIVEN there is a file in the supergraph config root dir
        let full_schema_path =
            Utf8PathBuf::try_from(supergraph_config_root_dir.path().join("schema.graphql"))?;
        let _ = fs::File::create(full_schema_path.as_std_path())?;

        // GIVEN the Schema source is a relative path from the supergraph config root
        let schema = SchemaSource::File {
            file: Utf8PathBuf::from("./schema.graphql"),
        };
        let routing_url = Some("routing_url".to_string());

        // GIVEN we have an unresolved subgraph from the
        let unresolved_subgraph = UnresolvedSubgraph::new(
            "subgraph_name".to_string(),
            SubgraphConfig {
                routing_url: routing_url.clone(),
                schema: schema.clone(),
            },
        );

        // WHEN we lazily resolve an unresolved subgraph against the supergraph config root
        let result = LazilyResolvedSubgraph::resolve(
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
            unresolved_subgraph,
        );

        // THEN we have a SchemaSource::File resolved to the canonicalized file path
        assert_that!(result)
            .is_ok()
            .is_equal_to(LazilyResolvedSubgraph {
                routing_url,
                schema: SchemaSource::File {
                    file: full_schema_path.canonicalize_utf8()?,
                },
            });
        Ok(())
    }

    #[test]
    fn test_lazily_resolve_file_subgraph_failure() -> Result<()> {
        // GIVEN there is a supergraph config root somewhere
        let supergraph_config_root_dir = tempdir()?;

        // GIVEN there is a schema file outside of the supergraph config root dir
        let other_root_dir = tempdir()?;
        let full_schema_path = Utf8PathBuf::try_from(other_root_dir.path().join("schema.graphql"))?;
        let _ = fs::File::create(full_schema_path.as_std_path())?;

        // GIVEN the Schema source is a relative path from the supergraph config root
        let schema = SchemaSource::File {
            file: Utf8PathBuf::from("./schema.graphql"),
        };
        let routing_url = Some("routing_url".to_string());

        // GIVEN we have an unresolved subgraph from the
        let unresolved_subgraph = UnresolvedSubgraph::new(
            "subgraph_name".to_string(),
            SubgraphConfig {
                routing_url: routing_url.clone(),
                schema: schema.clone(),
            },
        );

        // WHEN we lazily resolve an unresolved subgraph against the supergraph config root
        let result = LazilyResolvedSubgraph::resolve(
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
            unresolved_subgraph,
        );

        // THEN we should receive an error that the path was unable to be resolved
        let subject = assert_that!(result).is_err().subject;
        let _ = if let ResolveSubgraphError::FileNotFound {
            subgraph_name,
            supergraph_yaml_path,
            path,
            ..
        } = subject
        {
            assert_that!(subgraph_name).is_equal_to(&"subgraph_name".to_string());
            assert_that!(supergraph_yaml_path).is_equal_to(
                &Utf8PathBuf::from_path_buf(supergraph_config_root_dir.path().to_path_buf())
                    .unwrap(),
            );
            assert_that!(path).is_equal_to(&PathBuf::from("./schema.graphql"));
        } else {
            panic!("error was not ResolveSubgraphError::FileNotFound");
        };
        Ok(())
    }
}
