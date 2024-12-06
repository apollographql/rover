use std::collections::HashMap;
use std::str::FromStr;

use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use apollo_parser::{cst, Parser};
use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use rover_std::Fs;
use url::Url;

use crate::composition::supergraph::config::lazy::LazilyResolvedSubgraph;
use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError, unresolved::UnresolvedSubgraph,
    },
    utils::effect::{fetch_remote_subgraph::FetchRemoteSubgraph, introspect::IntrospectSubgraph},
};

/// Represents a [`SubgraphConfig`] that has been resolved down to an SDL
#[derive(Clone, Debug, Eq, PartialEq, Getters)]
pub struct FullyResolvedSubgraph {
    routing_url: Option<String>,
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
                Ok(Self::resolve_file_schema(
                    unresolved_subgraph.routing_url,
                    &file,
                )?)
            }
            SchemaSource::SubgraphIntrospection {
                subgraph_url,
                introspection_headers,
            } => Ok(Self::resolve_subgraph_introspection(
                introspect_subgraph_impl,
                unresolved_subgraph.name.clone(),
                unresolved_subgraph.routing_url.clone(),
                subgraph_url,
                introspection_headers,
            )
            .await?),
            SchemaSource::Subgraph {
                graphref: graph_ref,
                subgraph,
            } => Ok(Self::resolve_subgraph(
                fetch_remote_subgraph_impl,
                unresolved_subgraph.routing_url.clone(),
                graph_ref,
                subgraph,
            )
            .await?),
            SchemaSource::Sdl { sdl } => Self::resolve_sdl(sdl),
        }
    }

    /// Fully resolves a [`LazilyResolvedSubgraph`] to a [`FullyResolvedSubgraph`]
    pub async fn fully_resolve(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        lazily_resolved_subgraph: LazilyResolvedSubgraph,
        subgraph_name: String,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        match &lazily_resolved_subgraph.schema {
            SchemaSource::File { file } => {
                Self::resolve_file_schema(lazily_resolved_subgraph.routing_url, file)
            }
            SchemaSource::SubgraphIntrospection {
                subgraph_url,
                introspection_headers,
            } => {
                Self::resolve_subgraph_introspection(
                    introspect_subgraph_impl,
                    subgraph_name,
                    lazily_resolved_subgraph.routing_url,
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
                    lazily_resolved_subgraph.routing_url,
                    graph_ref,
                    subgraph,
                )
                .await
            }
            SchemaSource::Sdl { sdl } => Self::resolve_sdl(sdl),
        }
    }

    fn resolve_file_schema(
        routing_url: Option<String>,
        file: &Utf8PathBuf,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        let schema = Fs::read_file(file).map_err(|err| ResolveSubgraphError::Fs(Box::new(err)))?;
        let is_fed_two = schema_contains_link_directive(&schema);
        Ok(FullyResolvedSubgraph {
            routing_url: routing_url.clone(),
            schema,
            is_fed_two,
        })
    }

    async fn resolve_subgraph(
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        routing_url: Option<String>,
        graph_ref: &str,
        subgraph: &String,
    ) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        let graph_ref =
            GraphRef::from_str(graph_ref).map_err(|err| ResolveSubgraphError::InvalidGraphRef {
                graph_ref: graph_ref.to_owned(),
                source: Box::new(err),
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
            routing_url: routing_url
                .clone()
                .or(Some(remote_subgraph.routing_url().to_string())),
            schema,
            is_fed_two,
        })
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
        let routing_url = routing_url
            .clone()
            .or_else(|| Some(subgraph_url.to_string()));
        let is_fed_two = schema_contains_link_directive(&schema);
        Ok(FullyResolvedSubgraph {
            routing_url,
            schema,
            is_fed_two,
        })
    }
    fn resolve_sdl(sdl: &String) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        let is_fed_two = schema_contains_link_directive(sdl);
        Ok(FullyResolvedSubgraph {
            routing_url: None,
            schema: sdl.to_string(),
            is_fed_two,
        })
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

impl From<LazilyResolvedSubgraph> for SchemaSource {
    fn from(value: LazilyResolvedSubgraph) -> Self {
        value.schema
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
