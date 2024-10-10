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
    Fs(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to introspect the subgraph {name}.")]
    IntrospectionError {
        name: String,
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Invalid graph ref: {graph_ref}")]
    InvalidGraphRef {
        graph_ref: String,
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to fetch the sdl for subgraph `{name}` from remote")]
    FetchRemoteSdlError {
        name: String,
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error(
        "The subgraph `{name}` with graph ref `{graph_ref}` does not have an assigned routing url"
    )]
    MissingRoutingUrl { name: String, graph_ref: GraphRef },
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug, Eq, PartialEq, Getters)]
pub struct FullyResolvedSubgraph {
    #[getter(skip)]
    routing_url: Option<String>,
    #[getter(skip)]
    schema: String,
    is_fed_two: bool,
}

#[buildstructor]
impl FullyResolvedSubgraph {
    #[builder]
    pub fn new(
        schema: String,
        routing_url: Option<String>,
        is_fed_two: Option<bool>,
    ) -> FullyResolvedSubgraph {
        FullyResolvedSubgraph {
            schema,
            routing_url,
            is_fed_two: is_fed_two.unwrap_or_default(),
        }
    }
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
                let is_fed_two = schema_contains_link_directive(&schema);
                Ok(FullyResolvedSubgraph {
                    routing_url: unresolved_subgraph.routing_url.clone(),
                    schema,
                    is_fed_two,
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
                let is_fed_two = schema_contains_link_directive(&schema);
                Ok(FullyResolvedSubgraph {
                    routing_url,
                    schema,
                    is_fed_two,
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
                let schema = remote_subgraph.schema().clone();
                let is_fed_two = schema_contains_link_directive(&schema);
                Ok(FullyResolvedSubgraph {
                    routing_url: unresolved_subgraph
                        .routing_url
                        .clone()
                        .or(Some(remote_subgraph.routing_url().to_string())),
                    schema,
                    is_fed_two,
                })
            }
            SchemaSource::Sdl { sdl } => {
                let is_fed_two = schema_contains_link_directive(sdl);
                Ok(FullyResolvedSubgraph {
                    routing_url: None,
                    schema: sdl.to_string(),
                    is_fed_two,
                })
            }
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

#[derive(Clone, Debug, Eq, PartialEq, Getters, Builder)]
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

fn schema_contains_link_directive(sdl: &str) -> bool {
    let parser = Parser::new(sdl);
    let parsed_ast = parser.parse();
    let doc = parsed_ast.document();
    doc.definitions().any(|definition| {
        match definition {
            cst::Definition::SchemaExtension(ext) => ext.directives(),
            cst::Definition::SchemaDefinition(def) => def.directives(),
            _ => None,
        }
        .map(|d| d.directives())
        .map(|mut directives| {
            directives.any(|directive| {
                directive
                    .name()
                    .map(|name| "link" == name.text())
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default()
    })
}

#[cfg(test)]
pub(crate) mod scenario {
    use std::{collections::HashMap, io::Write, path::Path, str::FromStr};

    use anyhow::Result;
    use apollo_federation_types::config::SchemaSource;
    use camino::Utf8PathBuf;
    use rand::Rng;
    use rover_client::shared::GraphRef;
    use rstest::fixture;
    use uuid::Uuid;

    use super::UnresolvedSubgraph;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum SubgraphFederationVersion {
        One,
        Two,
    }

    impl SubgraphFederationVersion {
        pub fn is_fed_two(&self) -> bool {
            matches!(self, SubgraphFederationVersion::Two)
        }
    }

    fn graph_id_or_variant() -> String {
        const ALPHA_CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        const ADDITIONAL_CHARSET: &[u8] =
            b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
        let mut rng = rand::thread_rng();
        let mut value = format!(
            "{}",
            ALPHA_CHARSET[rng.gen_range(0..ALPHA_CHARSET.len())] as char
        );
        let remaining = rng.gen_range(0..62);
        for _ in 0..remaining {
            let c = ADDITIONAL_CHARSET[rng.gen_range(0..ADDITIONAL_CHARSET.len())] as char;
            value.push(c);
        }
        value
    }

    #[fixture]
    pub fn graph_ref() -> GraphRef {
        let graph = graph_id_or_variant();
        let variant = graph_id_or_variant();
        GraphRef::from_str(&format!("{graph}@{variant}")).unwrap()
    }

    #[fixture]
    pub fn subgraph_name() -> String {
        format!("subgraph_{}", Uuid::new_v4().as_simple())
    }

    #[fixture]
    pub fn sdl() -> String {
        format!(
            "type Query {{ test_{}: String! }}",
            Uuid::new_v4().as_simple()
        )
    }

    #[fixture]
    pub fn sdl_fed2(sdl: String) -> String {
        let link_directive = "extend schema @link(url: \"https://specs.apollo.dev/federation/v2.3\", import: [\"@key\", \"@shareable\"])";
        format!("{}\n{}", link_directive, sdl)
    }

    #[fixture]
    pub fn routing_url() -> String {
        format!("http://example.com/{}", Uuid::new_v4().as_simple())
    }

    #[derive(Clone, Debug)]
    pub struct SdlSubgraphScenario {
        pub sdl: String,
        pub unresolved_subgraph: UnresolvedSubgraph,
        pub subgraph_federation_version: SubgraphFederationVersion,
    }

    #[fixture]
    pub fn sdl_subgraph_scenario(
        sdl: String,
        subgraph_name: String,
        #[default(SubgraphFederationVersion::One)]
        subgraph_federation_version: SubgraphFederationVersion,
    ) -> SdlSubgraphScenario {
        let sdl = if subgraph_federation_version.is_fed_two() {
            sdl_fed2(sdl)
        } else {
            sdl
        };
        SdlSubgraphScenario {
            sdl: sdl.to_string(),
            unresolved_subgraph: UnresolvedSubgraph {
                name: subgraph_name,
                routing_url: None,
                schema: SchemaSource::Sdl { sdl },
            },
            subgraph_federation_version,
        }
    }

    #[derive(Clone, Debug)]
    pub struct RemoteSubgraphScenario {
        pub sdl: String,
        pub graph_ref: GraphRef,
        pub unresolved_subgraph: UnresolvedSubgraph,
        pub subgraph_name: String,
        pub routing_url: String,
        pub subgraph_federation_version: SubgraphFederationVersion,
    }

    #[fixture]
    pub fn remote_subgraph_scenario(
        sdl: String,
        subgraph_name: String,
        routing_url: String,
        #[default(SubgraphFederationVersion::One)]
        subgraph_federation_version: SubgraphFederationVersion,
    ) -> RemoteSubgraphScenario {
        let graph_ref = graph_ref();
        let sdl = if subgraph_federation_version.is_fed_two() {
            sdl_fed2(sdl)
        } else {
            sdl
        };
        RemoteSubgraphScenario {
            sdl,
            graph_ref: graph_ref.clone(),
            unresolved_subgraph: UnresolvedSubgraph {
                name: subgraph_name.to_string(),
                schema: SchemaSource::Subgraph {
                    graphref: graph_ref.to_string(),
                    subgraph: subgraph_name.to_string(),
                },
                routing_url: Some(routing_url.to_string()),
            },
            subgraph_name,
            routing_url,
            subgraph_federation_version,
        }
    }

    #[derive(Clone, Debug)]
    pub struct IntrospectSubgraphScenario {
        pub sdl: String,
        pub routing_url: String,
        pub introspection_headers: HashMap<String, String>,
        pub unresolved_subgraph: UnresolvedSubgraph,
        pub subgraph_federation_version: SubgraphFederationVersion,
    }

    #[fixture]
    pub fn introspect_subgraph_scenario(
        sdl: String,
        subgraph_name: String,
        routing_url: String,
        #[default(SubgraphFederationVersion::One)]
        subgraph_federation_version: SubgraphFederationVersion,
    ) -> IntrospectSubgraphScenario {
        let sdl = if subgraph_federation_version.is_fed_two() {
            sdl_fed2(sdl)
        } else {
            sdl
        };
        let introspection_headers = HashMap::from_iter([(
            "x-introspection-key".to_string(),
            "x-introspection-header".to_string(),
        )]);
        IntrospectSubgraphScenario {
            sdl,
            routing_url: routing_url.to_string(),
            introspection_headers: introspection_headers.clone(),
            unresolved_subgraph: UnresolvedSubgraph {
                name: subgraph_name,
                schema: SchemaSource::SubgraphIntrospection {
                    subgraph_url: url::Url::from_str(&routing_url).unwrap(),
                    introspection_headers: Some(introspection_headers),
                },
                routing_url: Some(routing_url),
            },
            subgraph_federation_version,
        }
    }

    #[derive(Clone, Debug)]
    pub struct FileSubgraphScenario {
        pub sdl: String,
        pub subgraph_name: String,
        pub routing_url: String,
        pub schema_file_path: Utf8PathBuf,
        pub unresolved_subgraph: UnresolvedSubgraph,
        pub subgraph_federation_version: SubgraphFederationVersion,
    }

    impl FileSubgraphScenario {
        pub fn write_schema_file(&self, root_dir: &Path) -> Result<()> {
            let full_schema_path = Utf8PathBuf::try_from(root_dir.join(&self.schema_file_path))?;
            let mut file = std::fs::File::create(full_schema_path.as_std_path())?;
            file.write_all(self.sdl.as_bytes())?;
            Ok(())
        }
    }

    #[fixture]
    pub fn file_subgraph_scenario(
        sdl: String,
        subgraph_name: String,
        routing_url: String,
        #[default(SubgraphFederationVersion::One)]
        subgraph_federation_version: SubgraphFederationVersion,
    ) -> FileSubgraphScenario {
        let sdl = if subgraph_federation_version.is_fed_two() {
            sdl_fed2(sdl)
        } else {
            sdl
        };
        let schema_file_path = Utf8PathBuf::from_str("schema.graphql").unwrap();
        FileSubgraphScenario {
            sdl,
            subgraph_name: subgraph_name.to_string(),
            routing_url: routing_url.clone(),
            schema_file_path: schema_file_path.clone(),
            unresolved_subgraph: UnresolvedSubgraph {
                name: subgraph_name,
                schema: SchemaSource::File {
                    file: schema_file_path,
                },
                routing_url: Some(routing_url),
            },
            subgraph_federation_version,
        }
    }
}

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
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
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
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
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
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
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
            &Utf8PathBuf::try_from(supergraph_config_root_dir.path().to_path_buf())?,
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
        let _ = if let ResolveSubgraphError::FileNotFound {
            subgraph_name: actual_subgraph_name,
            supergraph_yaml_path,
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
