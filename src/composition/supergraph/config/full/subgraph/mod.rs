use std::str::FromStr;

use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use apollo_parser::{cst, Parser};
use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use tower::{service_fn, util::BoxCloneService, Service, ServiceExt};

pub mod file;
pub mod introspect;
pub mod remote;

use crate::composition::supergraph::config::{
    error::ResolveSubgraphError, resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory,
    unresolved::UnresolvedSubgraph,
};

use self::{
    file::ResolveFileSubgraph,
    introspect::{MakeResolveIntrospectSubgraphRequest, ResolveIntrospectSubgraphFactory},
    remote::ResolveRemoteSubgraph,
};

/// Alias for a [`tower::Service`] that fully resolves a subgraph
pub type FullyResolveSubgraphService =
    BoxCloneService<(), FullyResolvedSubgraph, ResolveSubgraphError>;

/// Represents a [`SubgraphConfig`] that has been resolved down to an SDL
#[derive(Clone, Debug, Eq, PartialEq, Getters)]
pub struct FullyResolvedSubgraph {
    name: String,
    routing_url: String,
    schema: String,
    pub(crate) is_fed_two: bool,
}

#[buildstructor]
impl FullyResolvedSubgraph {
    /// Hook for [`buildstructor::buildstructor`]'s builder pattern to create a [`FullyResolvedSubgraph`]
    #[builder]
    pub fn new(name: String, schema: String, routing_url: String) -> FullyResolvedSubgraph {
        let is_fed_two = schema_contains_link_directive(&schema);
        FullyResolvedSubgraph {
            name,
            schema,
            routing_url,
            is_fed_two,
        }
    }

    /// Resolves a [`UnresolvedSubgraph`] to a [`FullyResolvedSubgraph`]
    pub async fn resolver(
        mut resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        mut fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        supergraph_config_root: &Utf8PathBuf,
        unresolved_subgraph: impl Into<UnresolvedSubgraph>,
    ) -> Result<FullyResolveSubgraphService, ResolveSubgraphError> {
        let unresolved_subgraph = unresolved_subgraph.into();
        let schema = unresolved_subgraph.schema().clone();
        match schema {
            SchemaSource::File { file } => {
                let service = ResolveFileSubgraph::builder()
                    .supergraph_config_root(supergraph_config_root)
                    .path(file.clone())
                    .unresolved_subgraph(unresolved_subgraph.clone())
                    .build();
                Ok(service.boxed_clone())
            }
            SchemaSource::SubgraphIntrospection {
                subgraph_url,
                introspection_headers,
            } => {
                let request = MakeResolveIntrospectSubgraphRequest::builder()
                    .headers(introspection_headers.clone().unwrap_or_default())
                    .endpoint(subgraph_url.clone())
                    .and_routing_url(unresolved_subgraph.routing_url().clone())
                    .subgraph_name(unresolved_subgraph.name().to_string())
                    .build();
                let service = resolve_introspect_subgraph_factory.ready().await?;
                let service = service.call(request).await?;
                Ok(service.boxed_clone())
            }
            SchemaSource::Subgraph {
                graphref: graph_ref,
                subgraph,
            } => {
                let graph_ref = GraphRef::from_str(&graph_ref).map_err(|err| {
                    ResolveSubgraphError::InvalidGraphRef {
                        graph_ref: graph_ref.clone(),
                        source: Box::new(err),
                    }
                })?;

                let fetch_remote_subgraph_factory = fetch_remote_subgraph_factory
                    .ready()
                    .await
                    .map_err(|err| ResolveSubgraphError::ServiceReady(Box::new(err)))?;

                let service = fetch_remote_subgraph_factory
                    .call(())
                    .await
                    .map_err(|err| ResolveSubgraphError::FetchRemoteSdlError {
                        subgraph_name: subgraph.to_string(),
                        source: Box::new(err),
                    })?;
                let service = ResolveRemoteSubgraph::builder()
                    .graph_ref(graph_ref)
                    .subgraph_name(subgraph.to_string())
                    .and_routing_url(unresolved_subgraph.routing_url().clone())
                    .inner(service)
                    .build();
                Ok(service.boxed_clone())
            }
            SchemaSource::Sdl { sdl } => Ok(service_fn(move |_: ()| {
                let unresolved_subgraph = unresolved_subgraph.clone();
                let sdl = sdl.to_string();
                async move {
                    Ok(FullyResolvedSubgraph::builder()
                        .name(unresolved_subgraph.name().to_string())
                        .routing_url(unresolved_subgraph.routing_url().clone().ok_or_else(
                            || ResolveSubgraphError::MissingRoutingUrl {
                                subgraph: unresolved_subgraph.name().to_string(),
                            },
                        )?)
                        .schema(sdl.to_string())
                        .build())
                }
            })
            .boxed_clone()),
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
            routing_url: Some(value.routing_url),
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
