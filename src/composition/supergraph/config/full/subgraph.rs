use std::str::FromStr;

use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use apollo_parser::{cst, Parser};
use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use rover_std::Fs;

use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError, unresolved::UnresolvedSubgraph,
    },
    utils::effect::{fetch_remote_subgraph::FetchRemoteSubgraph, introspect::IntrospectSubgraph},
};

/// Represents a [`SubgraphConfig`] that has been resolved down to an SDL
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
    /// Hook for [`buildstructor::buildstructor`]'s builder pattern to create a [`FullyResolvedSubgraph`]
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
    /// Resolves a [`UnresolvedSubgraph`] to a [`FullyResolvedSubgraph`]
    pub async fn resolve(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        supergraph_config_root: Option<&Utf8PathBuf>,
        unresolved_subgraph: UnresolvedSubgraph,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        match unresolved_subgraph.schema() {
            SchemaSource::File { file } => {
                let supergraph_config_root =
                    supergraph_config_root.ok_or(ResolveSubgraphError::SupergraphConfigMissing)?;
                let file = unresolved_subgraph.resolve_file_path(supergraph_config_root, file)?;
                let schema =
                    Fs::read_file(&file).map_err(|err| ResolveSubgraphError::Fs(Box::new(err)))?;
                let is_fed_two = schema_contains_link_directive(&schema);
                Ok(FullyResolvedSubgraph {
                    routing_url: unresolved_subgraph.routing_url().clone(),
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
                        subgraph_name: unresolved_subgraph.name().to_string(),
                        source: Box::new(err),
                    })?;
                let routing_url = unresolved_subgraph
                    .routing_url()
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
                        source: Box::new(err),
                    }
                })?;
                let remote_subgraph = fetch_remote_subgraph_impl
                    .fetch_remote_subgraph(graph_ref, subgraph.to_string())
                    .await
                    .map_err(|err| ResolveSubgraphError::FetchRemoteSdlError {
                        subgraph_name: subgraph.to_string(),
                        source: Box::new(err),
                    })?;
                let schema = remote_subgraph.schema().clone();
                let is_fed_two = schema_contains_link_directive(&schema);
                Ok(FullyResolvedSubgraph {
                    routing_url: unresolved_subgraph
                        .routing_url()
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
