use std::str::FromStr;

use anyhow::anyhow;
use apollo_federation_types::{
    config::{FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig},
    javascript::SubgraphDefinition,
    rover::{BuildError, BuildErrors},
};
use apollo_parser::{cst, Parser};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use rover_client::operations::subgraph::fetch::{self, SubgraphFetchInput};
use rover_client::operations::subgraph::introspect::{self, SubgraphIntrospectInput};
use rover_client::shared::GraphRef;
use rover_client::{blocking::GraphQLClient, RoverClientError};
use rover_std::{Fs, Style};

use crate::{
    options::ProfileOpt,
    utils::{client::StudioClientConfig, expansion::expand, parsers::FileDescriptorType},
};
use crate::{RoverError, RoverErrorSuggestion, RoverResult};

pub(crate) fn expand_supergraph_yaml(content: &str) -> RoverResult<SupergraphConfig> {
    serde_yaml::from_str(content)
        .map_err(RoverError::from)
        .and_then(expand)
        .and_then(|v| serde_yaml::from_value(v).map_err(RoverError::from))
}

pub(crate) fn resolve_supergraph_yaml(
    unresolved_supergraph_yaml: &FileDescriptorType,
    client_config: StudioClientConfig,
    profile_opt: &ProfileOpt,
) -> RoverResult<SupergraphConfig> {
    let err_no_routing_url = || {
        let err = anyhow!("No routing_url found for schema file.");
        let mut err = RoverError::new(err);
        err.set_suggestion(RoverErrorSuggestion::ValidComposeRoutingUrl);
        err
    };
    let supergraph_config = unresolved_supergraph_yaml
        .read_file_descriptor("supergraph config", &mut std::io::stdin())
        .and_then(|contents| expand_supergraph_yaml(&contents))?;
    let maybe_specified_federation_version = supergraph_config.get_federation_version();
    let supergraph_config = supergraph_config
        .into_iter()
        .collect::<Vec<(String, SubgraphConfig)>>();

    let subgraph_definition_results: Vec<(String, RoverResult<SubgraphDefinition>)> =
        supergraph_config
            .into_par_iter()
            .map(|(name, subgraph_data)| {
                let cloned_subgraph_name = name.clone();
                let result = match &subgraph_data.schema {
                    SchemaSource::File { file } => {
                        let relative_schema_path = match unresolved_supergraph_yaml {
                            FileDescriptorType::File(config_path) => match config_path.parent() {
                                Some(parent) => {
                                    let mut schema_path = parent.to_path_buf();
                                    schema_path.push(file);
                                    schema_path
                                }
                                None => file.clone(),
                            },
                            FileDescriptorType::Stdin => file.clone(),
                        };

                        Fs::read_file(relative_schema_path)
                            .map_err(|e| {
                                let mut err = RoverError::new(e);
                                err.set_suggestion(RoverErrorSuggestion::ValidComposeFile);
                                err
                            })
                            .and_then(|sdl| {
                                subgraph_data
                                    .routing_url
                                    .clone()
                                    .ok_or_else(err_no_routing_url)
                                    .map(|url| SubgraphDefinition { name, url, sdl })
                            })
                    }
                    SchemaSource::SubgraphIntrospection {
                        subgraph_url,
                        introspection_headers,
                    } => {
                        client_config
                            .get_reqwest_client()
                            .map_err(RoverError::from)
                            .and_then(|reqwest_client| {
                                let client =
                                    GraphQLClient::new(subgraph_url.as_ref(), reqwest_client);

                                // given a federated introspection URL, use subgraph introspect to
                                // obtain SDL and add it to subgraph_definition.
                                introspect::run(
                                    SubgraphIntrospectInput {
                                        headers: introspection_headers.clone().unwrap_or_default(),
                                    },
                                    &client,
                                    false,
                                )
                                .map(|introspection_response| {
                                    let sdl = introspection_response.result;

                                    // We don't require a routing_url in config for this variant of a schema,
                                    // if one isn't provided, just use the URL they passed for introspection.
                                    let url = subgraph_data
                                        .routing_url
                                        .unwrap_or_else(|| subgraph_url.to_string());
                                    SubgraphDefinition { name, url, sdl }
                                })
                                .map_err(RoverError::from)
                            })
                    }
                    SchemaSource::Subgraph {
                        graphref: graph_ref,
                        subgraph,
                    } => {
                        client_config
                            .get_authenticated_client(profile_opt)
                            .map_err(RoverError::from)
                            .and_then(|authenticated_client| {
                                // given a graph_ref and subgraph, run subgraph fetch to
                                // obtain SDL and add it to subgraph_definition.
                                fetch::run(
                                    SubgraphFetchInput {
                                        graph_ref: GraphRef::from_str(graph_ref)?,
                                        subgraph_name: subgraph.clone(),
                                    },
                                    &authenticated_client,
                                )
                                .map_err(RoverError::from)
                                .and_then(|result| {
                                    // We don't require a routing_url in config for this variant of a schema,
                                    // if one isn't provided, just use the routing URL from the graph registry (if it exists).
                                    if let rover_client::shared::SdlType::Subgraph {
                                        routing_url: Some(graph_registry_routing_url),
                                    } = result.sdl.r#type
                                    {
                                        let url = subgraph_data
                                            .routing_url
                                            .clone()
                                            .unwrap_or(graph_registry_routing_url);
                                        Ok(SubgraphDefinition {
                                            name,
                                            url,
                                            sdl: result.sdl.contents,
                                        })
                                    } else {
                                        Err(err_no_routing_url())
                                    }
                                })
                            })
                    }
                    SchemaSource::Sdl { sdl } => subgraph_data
                        .routing_url
                        .ok_or_else(err_no_routing_url)
                        .map(|url| SubgraphDefinition {
                            name,
                            url,
                            sdl: sdl.clone(),
                        }),
                };

                (cloned_subgraph_name, result)
            })
            .collect();

    let mut subgraph_definitions = Vec::new();
    let mut subgraph_definition_errors = Vec::new();

    let num_subgraphs = subgraph_definition_results.len();

    for (subgraph_name, subgraph_definition_result) in subgraph_definition_results {
        match subgraph_definition_result {
            Ok(subgraph_definition) => subgraph_definitions.push(subgraph_definition),
            Err(e) => subgraph_definition_errors.push((subgraph_name, e)),
        }
    }

    if !subgraph_definition_errors.is_empty() {
        let source = BuildErrors::from(
            subgraph_definition_errors
                .iter()
                .map(|(subgraph_name, error)| {
                    let mut message = error.message();
                    if message.ends_with('.') {
                        message.pop();
                    }
                    let mut message = format!(
                        "{} while resolving the schema for the '{}' subgraph",
                        message, subgraph_name
                    );
                    for suggestion in error.suggestions() {
                        message = format!("{}\n        {}", message, suggestion)
                    }
                    BuildError::config_error(error.code().map(|c| format!("{}", c)), Some(message))
                })
                .collect::<Vec<BuildError>>(),
        );
        return Err(RoverError::from(RoverClientError::BuildErrors {
            source,
            num_subgraphs,
        }));
    }

    let mut resolved_supergraph_config: SupergraphConfig = subgraph_definitions.into();

    let mut fed_two_subgraph_names = Vec::new();
    for subgraph_definition in resolved_supergraph_config.get_subgraph_definitions()? {
        let parser = Parser::new(&subgraph_definition.sdl);
        let parsed_ast = parser.parse();
        let doc = parsed_ast.document();
        for definition in doc.definitions() {
            let maybe_directives = match definition {
                cst::Definition::SchemaExtension(ext) => ext.directives(),
                cst::Definition::SchemaDefinition(def) => def.directives(),
                _ => None,
            }
            .map(|d| d.directives());
            if let Some(directives) = maybe_directives {
                for directive in directives {
                    if let Some(directive_name) = directive.name() {
                        if "link" == directive_name.text() {
                            fed_two_subgraph_names.push(subgraph_definition.name.clone());
                        }
                    }
                }
            }
        }
    }

    let print_inexact_warning = || {
        eprintln!("{} An exact {} was not specified in '{}'. Future versions of {} will fail without specifying an exact federation version. See {} for more information.", Style::WarningPrefix.paint("WARN:"), Style::Command.paint("federation_version"), &unresolved_supergraph_yaml, Style::Command.paint("`rover supergraph compose`"), Style::Link.paint("https://www.apollographql.com/docs/rover/commands/supergraphs#setting-a-composition-version"))
    };

    if let Some(specified_federation_version) = maybe_specified_federation_version {
        // error if we detect an `@link` directive and the explicitly set `federation_version` to 1
        if specified_federation_version.is_fed_one() && !fed_two_subgraph_names.is_empty() {
            let mut err =
                RoverError::new(anyhow!("The 'federation_version' set in '{}' is invalid. The following subgraphs contain '@link' directives, which are only valid in Federation 2: {}", unresolved_supergraph_yaml, fed_two_subgraph_names.join(", ")));
            err.set_suggestion(RoverErrorSuggestion::Adhoc(format!(
                "Either remove the 'federation_version' entry from '{}', or set the value to '2'.",
                unresolved_supergraph_yaml
            )));
            return Err(err);
        }

        if matches!(
            specified_federation_version,
            FederationVersion::LatestFedOne
        ) || matches!(
            specified_federation_version,
            FederationVersion::LatestFedTwo
        ) {
            print_inexact_warning();
        }

        // otherwise, set the version to what they set
        resolved_supergraph_config.set_federation_version(specified_federation_version)
    } else if fed_two_subgraph_names.is_empty() {
        // if they did not specify a version and no subgraphs contain `@link` directives, use Federation 1
        print_inexact_warning();
        resolved_supergraph_config.set_federation_version(FederationVersion::LatestFedOne)
    } else {
        // if they did not specify a version and at least one subgraph contains an `@link` directive, use Federation 2
        print_inexact_warning();
        resolved_supergraph_config.set_federation_version(FederationVersion::LatestFedTwo)
    }

    Ok(resolved_supergraph_config)
}

#[cfg(test)]
mod test_expand_supergraph_yaml {
    use apollo_federation_types::config::FederationVersion;

    #[test]
    fn test_supergraph_yaml_int_version() {
        let yaml = r#"
federation_version: 1
subgraphs: 
"#;
        let config = super::expand_supergraph_yaml(yaml).unwrap();
        assert_eq!(
            config.get_federation_version(),
            Some(FederationVersion::LatestFedOne)
        );
    }
}
