use camino::Utf8PathBuf;

use apollo_federation_types::{
    build::SubgraphDefinition,
    config::{SchemaSource, SupergraphConfig},
};

use std::{collections::HashMap, fs, str::FromStr};

use rover_client::blocking::GraphQLClient;
use rover_client::operations::subgraph::fetch::{self, SubgraphFetchInput};
use rover_client::operations::subgraph::introspect::{self, SubgraphIntrospectInput};
use rover_client::shared::GraphRef;

use crate::utils::client::StudioClientConfig;
use crate::{anyhow, error::RoverError, Result, Suggestion};

pub(crate) fn resolve_supergraph_yaml(
    config_path: &Utf8PathBuf,
    client_config: StudioClientConfig,
    profile_name: &str,
) -> Result<SupergraphConfig> {
    let mut subgraph_definitions = Vec::new();

    let err_no_routing_url = || {
        let err = anyhow!("No routing_url found for schema file.");
        let mut err = RoverError::new(err);
        err.set_suggestion(Suggestion::ValidComposeRoutingUrl);
        err
    };

    let supergraph_config = SupergraphConfig::new_from_yaml_file(config_path)?;
    let federation_version = supergraph_config.get_federation_version().to_string();

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

                let schema = fs::read_to_string(&relative_schema_path).map_err(|e| {
                    let err = anyhow!("Could not read \"{}\": {}", &relative_schema_path, e);
                    let mut err = RoverError::new(err);
                    err.set_suggestion(Suggestion::ValidComposeFile);
                    err
                })?;

                let url = &subgraph_data
                    .routing_url
                    .clone()
                    .ok_or_else(err_no_routing_url)?;

                subgraph_definitions.push(SubgraphDefinition::new(subgraph_name, url, &schema));
            }
            SchemaSource::SubgraphIntrospection { subgraph_url } => {
                // given a federated introspection URL, use subgraph introspect to
                // obtain SDL and add it to subgraph_definition.
                let client =
                    GraphQLClient::new(subgraph_url.as_ref(), client_config.get_reqwest_client())?;

                let introspection_response = introspect::run(
                    SubgraphIntrospectInput {
                        headers: HashMap::new(),
                    },
                    &client,
                )?;
                let schema = introspection_response.result;

                // We don't require a routing_url in config for this variant of a schema,
                // if one isn't provided, just use the URL they passed for introspection.
                let url = &subgraph_data
                    .routing_url
                    .clone()
                    .unwrap_or_else(|| subgraph_url.to_string());

                subgraph_definitions.push(SubgraphDefinition::new(subgraph_name, url, &schema));
            }
            SchemaSource::Subgraph {
                graphref: graph_ref,
                subgraph,
            } => {
                // given a graph_ref and subgraph, run subgraph fetch to
                // obtain SDL and add it to subgraph_definition.
                let client = client_config.get_authenticated_client(profile_name)?;
                let result = fetch::run(
                    SubgraphFetchInput {
                        graph_ref: GraphRef::from_str(graph_ref)?,
                        subgraph_name: subgraph.clone(),
                    },
                    &client,
                )?;

                // We don't require a routing_url in config for this variant of a schema,
                // if one isn't provided, just use the routing URL from the graph registry (if it exists).
                let url = if let rover_client::shared::SdlType::Subgraph {
                    routing_url: Some(graph_registry_routing_url),
                } = result.sdl.r#type
                {
                    Ok(subgraph_data
                        .routing_url
                        .clone()
                        .unwrap_or(graph_registry_routing_url))
                } else {
                    Err(err_no_routing_url())
                }?;

                subgraph_definitions.push(SubgraphDefinition::new(
                    subgraph_name,
                    url,
                    &result.sdl.contents,
                ));
            }
            SchemaSource::Sdl { sdl } => {
                let url = &subgraph_data
                    .routing_url
                    .clone()
                    .ok_or_else(err_no_routing_url)?;
                subgraph_definitions.push(SubgraphDefinition::new(subgraph_name, url, sdl))
            }
        }
    }

    let mut resolved_supergraph_config: SupergraphConfig = subgraph_definitions.into();
    resolved_supergraph_config.set_federation_version(&federation_version)?;
    Ok(resolved_supergraph_config)
}
