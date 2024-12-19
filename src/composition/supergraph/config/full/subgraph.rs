use std::collections::HashMap;
use std::str::FromStr;

use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use apollo_parser::{cst, Parser};
use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use rover_std::Fs;
use tower::{MakeService, Service, ServiceExt};
use url::Url;

use crate::composition::supergraph::config::lazy::LazilyResolvedSubgraph;
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
        fetch_remote_subgraph_impl: MakeFetchSubgraph,
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
                Self::resolve_file(unresolved_subgraph.routing_url().clone(), &file)
            }
            SchemaSource::SubgraphIntrospection {
                subgraph_url,
                introspection_headers,
            } => {
                Self::resolve_subgraph_introspection(
                    introspect_subgraph_impl,
                    unresolved_subgraph.name().clone(),
                    unresolved_subgraph.routing_url().clone(),
                    subgraph_url,
                    introspection_headers,
                )
                .await
            }
            SchemaSource::Subgraph {
                graphref: graph_ref,
                subgraph,
            } => {
                Self::resolve_subgraph(
                    fetch_remote_subgraph_impl,
                    unresolved_subgraph.routing_url().clone(),
                    graph_ref,
                    subgraph,
                )
                .await
            }
            SchemaSource::Sdl { sdl } => {
                Self::resolve_sdl(unresolved_subgraph.routing_url().clone(), sdl)
            }
        }
    }

    /// Resolves a [`LazilyResolvedSubgraph`] to a [`FullyResolvedSubgraph`]
    pub async fn fully_resolve<MakeFetchSubgraph>(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: MakeFetchSubgraph,
        lazily_resolved_subgraph: LazilyResolvedSubgraph,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError>
    where
        MakeFetchSubgraph: MakeService<(), FetchRemoteSubgraphRequest, Response = RemoteSubgraph>,
        MakeFetchSubgraph::MakeError: std::error::Error + Send + Sync + 'static,
        MakeFetchSubgraph::Error: std::error::Error + Send + Sync + 'static,
    {
        match lazily_resolved_subgraph.schema() {
            SchemaSource::File { file } => {
                Self::resolve_file(lazily_resolved_subgraph.routing_url().clone(), file)
            }
            SchemaSource::SubgraphIntrospection {
                subgraph_url,
                introspection_headers,
            } => {
                Self::resolve_subgraph_introspection(
                    introspect_subgraph_impl,
                    lazily_resolved_subgraph.name().clone(),
                    lazily_resolved_subgraph.routing_url().clone(),
                    subgraph_url,
                    introspection_headers,
                )
                .await
            }
            SchemaSource::Subgraph {
                graphref: graph_ref,
                subgraph,
            } => {
                Self::resolve_subgraph(
                    fetch_remote_subgraph_impl,
                    lazily_resolved_subgraph.routing_url().clone(),
                    graph_ref,
                    subgraph,
                )
                .await
            }
            SchemaSource::Sdl { sdl } => {
                Self::resolve_sdl(lazily_resolved_subgraph.routing_url().clone(), sdl)
            }
        }
    }

    fn resolve_file(
        routing_url: Option<String>,
        file: &Utf8PathBuf,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        let schema = Fs::read_file(file).map_err(|err| ResolveSubgraphError::Fs(Box::new(err)))?;
        Ok(FullyResolvedSubgraph::builder()
            .and_routing_url(routing_url)
            .schema(schema)
            .build())
    }

    async fn resolve_subgraph_introspection(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        subgraph_name: String,
        routing_url: Option<String>,
        subgraph_url: &Url,
        introspection_headers: &Option<HashMap<String, String>>,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        let schema = introspect_subgraph_impl
            .introspect_subgraph(
                subgraph_url.clone(),
                introspection_headers.clone().unwrap_or_default(),
            )
            .await
            .map_err(|err| ResolveSubgraphError::IntrospectionError {
                subgraph_name,
                source: Box::new(err),
            })?;
        Ok(FullyResolvedSubgraph::builder()
            .and_routing_url(routing_url.or(Some(subgraph_url.to_string())))
            .schema(schema)
            .build())
    }

    async fn resolve_subgraph<MakeFetchSubgraph>(
        mut fetch_remote_subgraph_impl: MakeFetchSubgraph,
        routing_url: Option<String>,
        graph_ref: &str,
        subgraph: &String,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError>
    where
        MakeFetchSubgraph: MakeService<(), FetchRemoteSubgraphRequest, Response = RemoteSubgraph>,
        MakeFetchSubgraph::MakeError: std::error::Error + Send + Sync + 'static,
        MakeFetchSubgraph::Error: std::error::Error + Send + Sync + 'static,
    {
        let graph_ref =
            GraphRef::from_str(graph_ref).map_err(|err| ResolveSubgraphError::InvalidGraphRef {
                graph_ref: graph_ref.to_owned(),
                source: Box::new(err),
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
            .routing_url(routing_url.unwrap_or(remote_subgraph.routing_url().to_string()))
            .schema(schema)
            .build())
    }

    fn resolve_sdl(
        routing_url: Option<String>,
        sdl: &String,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        Ok(FullyResolvedSubgraph::builder()
            .and_routing_url(routing_url)
            .schema(sdl.to_string())
            .build())
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
