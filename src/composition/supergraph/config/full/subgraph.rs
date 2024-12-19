use std::str::FromStr;

use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use apollo_parser::{cst, Parser};
use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use rover_std::Fs;
use tower::{MakeService, Service, ServiceExt};

use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError,
        resolver::fetch_remote_subgraph::{FetchRemoteSubgraphRequest, RemoteSubgraph},
        unresolved::UnresolvedSubgraph,
    },
    utils::effect::introspect::IntrospectSubgraph,
};

/// Represents a [`SubgraphConfig`] that has been resolved down to an SDL
#[derive(Clone, Debug, Eq, PartialEq, Getters)]
pub struct FullyResolvedSubgraph {
    #[getter(skip)]
    routing_url: Option<String>,
    schema: String,
    is_fed_two: bool,
}

#[buildstructor]
impl FullyResolvedSubgraph {
    /// Hook for [`buildstructor::buildstructor`]'s builder pattern to create a [`FullyResolvedSubgraph`]
    #[builder]
    pub fn new(schema: String, routing_url: Option<String>) -> FullyResolvedSubgraph {
        let is_fed_two = schema_contains_link_directive(&schema);
        FullyResolvedSubgraph {
            schema,
            routing_url,
            is_fed_two,
        }
    }
    /// Resolves a [`UnresolvedSubgraph`] to a [`FullyResolvedSubgraph`]
    pub async fn resolve<MakeFetchSubgraph>(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        mut fetch_remote_subgraph_impl: MakeFetchSubgraph,
        supergraph_config_root: Option<&Utf8PathBuf>,
        unresolved_subgraph: UnresolvedSubgraph,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError>
    where
        MakeFetchSubgraph: MakeService<(), FetchRemoteSubgraphRequest, Response = RemoteSubgraph>,
        MakeFetchSubgraph::MakeError: std::error::Error + Send + Sync + 'static,
        MakeFetchSubgraph::Error: std::error::Error + Send + Sync + 'static,
    {
        match unresolved_subgraph.schema() {
            SchemaSource::File { file } => {
                let supergraph_config_root =
                    supergraph_config_root.ok_or(ResolveSubgraphError::SupergraphConfigMissing)?;
                let file = unresolved_subgraph.resolve_file_path(supergraph_config_root, file)?;
                let schema =
                    Fs::read_file(&file).map_err(|err| ResolveSubgraphError::Fs(Box::new(err)))?;
                Ok(FullyResolvedSubgraph::builder()
                    .and_routing_url(unresolved_subgraph.routing_url().clone())
                    .schema(schema)
                    .build())
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
                        subgraph_name: unresolved_subgraph.name().to_string(),
                        source: Box::new(err),
                    })?;
                let routing_url = unresolved_subgraph
                    .routing_url()
                    .clone()
                    .unwrap_or_else(|| subgraph_url.to_string());
                Ok(FullyResolvedSubgraph::builder()
                    .routing_url(routing_url)
                    .schema(schema)
                    .build())
            }
            SchemaSource::Subgraph {
                graphref: graph_ref,
                subgraph,
            } => {
                let graph_ref = GraphRef::from_str(graph_ref).map_err(|err| {
                    ResolveSubgraphError::InvalidGraphRef {
                        graph_ref: graph_ref.clone(),
                        source: Box::new(err),
                    }
                })?;
                let remote_subgraph = fetch_remote_subgraph_impl
                    .make_service(())
                    .await
                    .map_err(|err| ResolveSubgraphError::FetchRemoteSdlError {
                        subgraph_name: subgraph.to_string(),
                        source: Box::new(err),
                    })?
                    .ready()
                    .await
                    .map_err(|err| ResolveSubgraphError::FetchRemoteSdlError {
                        subgraph_name: subgraph.to_string(),
                        source: Box::new(err),
                    })?
                    .call(
                        FetchRemoteSubgraphRequest::builder()
                            .graph_ref(graph_ref)
                            .subgraph_name(subgraph.to_string())
                            .build(),
                    )
                    .await
                    .map_err(|err| ResolveSubgraphError::FetchRemoteSdlError {
                        subgraph_name: subgraph.to_string(),
                        source: Box::new(err),
                    })?;
                let schema = remote_subgraph.schema().clone();
                Ok(FullyResolvedSubgraph::builder()
                    .routing_url(
                        unresolved_subgraph
                            .routing_url()
                            .clone()
                            .unwrap_or_else(|| remote_subgraph.routing_url().to_string()),
                    )
                    .schema(schema)
                    .build())
            }
            SchemaSource::Sdl { sdl } => Ok(FullyResolvedSubgraph::builder()
                .and_routing_url(unresolved_subgraph.routing_url().clone())
                .schema(sdl.to_string())
                .build()),
        }
    }

    /// Mutably updates this subgraph's schema
    pub fn update_schema(&mut self, schema: String) {
        self.schema = schema;
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
