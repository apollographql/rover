use camino::Utf8PathBuf;

use apollo_federation_types::{
    build::{BuildError, BuildErrors, SubgraphDefinition},
    config::{SchemaSource, SupergraphConfig},
};

use std::{collections::HashMap, fs, str::FromStr};

use rover_client::blocking::GraphQLClient;
use rover_client::operations::subgraph::fetch::{self, SubgraphFetchInput};
use rover_client::operations::subgraph::introspect::{self, SubgraphIntrospectInput};
use rover_client::shared::GraphRef;

use crate::utils::client::StudioClientConfig;

pub(crate) fn resolve_supergraph_config(
    config_path: &Utf8PathBuf,
    client_config: StudioClientConfig,
    profile_name: &str,
) -> std::result::Result<Vec<SubgraphDefinition>, BuildErrors> {
    let mut subgraph_definitions = Vec::new();
    let mut build_errors = Vec::new();

    let err_no_routing_url = || {
        BuildError::config_error(
            None,
            Some("No routing_url found for schema file.".to_string()),
        )
    };

    let config_error = |message: String| BuildError::config_error(None, Some(message));

    let supergraph_config = SupergraphConfig::new_from_yaml_file(config_path)?;

    for (subgraph_name, subgraph_data) in supergraph_config.into_iter() {
        match &subgraph_data.schema {
            SchemaSource::File { file } => {
                let relative_schema_path = match config_path.parent() {
                    Some(parent) => {
                        let mut schema_path = parent.to_path_buf();
                        schema_path.push(file);
                        schema_path
                    }
                    None => file.clone(),
                };

                let maybe_schema = fs::read_to_string(&relative_schema_path);

                match maybe_schema {
                    Ok(schema) => {
                        if let Some(routing_url) = &subgraph_data.routing_url {
                            subgraph_definitions.push(SubgraphDefinition::new(
                                subgraph_name,
                                routing_url,
                                &schema,
                            ))
                        } else {
                            build_errors.push(err_no_routing_url());
                        }
                    }
                    Err(e) => {
                        let err = BuildError::config_error(
                            None,
                            Some(format!(
                                "Could not read \"{}\": {}",
                                &relative_schema_path, e
                            )),
                        );
                        build_errors.push(err);
                    }
                };
            }
            SchemaSource::SubgraphIntrospection { subgraph_url } => {
                // given a federated introspection URL, use subgraph introspect to
                // obtain SDL and add it to subgraph_definition.
                let client =
                    GraphQLClient::new(subgraph_url.as_ref(), client_config.get_reqwest_client());

                let maybe_introspection_response = introspect::run(
                    SubgraphIntrospectInput {
                        headers: HashMap::new(),
                    },
                    &client,
                );

                match maybe_introspection_response {
                    Ok(introspection_response) => {
                        let schema = introspection_response.result;

                        // We don't require a routing_url in config for this variant of a schema,
                        // if one isn't provided, just use the URL they passed for introspection.
                        let url = &subgraph_data
                            .routing_url
                            .clone()
                            .unwrap_or_else(|| subgraph_url.to_string());

                        subgraph_definitions.push(SubgraphDefinition::new(
                            subgraph_name,
                            url,
                            &schema,
                        ));
                    }
                    Err(e) => {
                        build_errors.push(config_error(e.to_string()));
                    }
                };
            }
            SchemaSource::Subgraph {
                graphref: graph_ref,
                subgraph,
            } => {
                // given a graph_ref and subgraph, run subgraph fetch to
                // obtain SDL and add it to subgraph_definition.
                let maybe_client = client_config.get_authenticated_client(profile_name);

                match maybe_client {
                    Ok(client) => {
                        let maybe_graph_ref = GraphRef::from_str(graph_ref);
                        match maybe_graph_ref {
                            Ok(graph_ref) => {
                                let maybe_fetch_result = fetch::run(
                                    SubgraphFetchInput {
                                        graph_ref,
                                        subgraph: subgraph.clone(),
                                    },
                                    &client,
                                );
                                match maybe_fetch_result {
                                    Ok(fetch_result) => {
                                        // We don't require a routing_url in config for this variant of a schema,
                                        // if one isn't provided, just use the routing URL from the graph registry (if it exists).
                                        if let rover_client::shared::SdlType::Subgraph {
                                            routing_url: Some(graph_registry_routing_url),
                                        } = fetch_result.sdl.r#type
                                        {
                                            let routing_url = subgraph_data
                                                .routing_url
                                                .clone()
                                                .unwrap_or(graph_registry_routing_url);
                                            subgraph_definitions.push(SubgraphDefinition::new(
                                                subgraph_name,
                                                routing_url,
                                                &fetch_result.sdl.contents,
                                            ));
                                        } else {
                                            build_errors.push(err_no_routing_url())
                                        };
                                    }
                                    Err(e) => build_errors.push(config_error(e.to_string())),
                                }
                            }
                            Err(e) => build_errors.push(config_error(e.to_string())),
                        }
                    }
                    Err(e) => build_errors.push(config_error(e.to_string())),
                };
            }
            SchemaSource::Sdl { sdl } => {
                if let Some(routing_url) = subgraph_data.routing_url {
                    subgraph_definitions.push(SubgraphDefinition::new(
                        subgraph_name,
                        routing_url,
                        sdl,
                    ));
                } else {
                    build_errors.push(err_no_routing_url());
                }
            }
        }
    }
    if build_errors.is_empty() {
        Ok(subgraph_definitions)
    } else {
        Err(build_errors.into())
    }
}
