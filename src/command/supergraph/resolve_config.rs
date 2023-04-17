use anyhow::anyhow;
use apollo_federation_types::{
    build::{BuildError, BuildErrors, SubgraphDefinition},
    config::{FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig},
};
use apollo_parser::{ast, Parser};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rover_std::{Fs, Style};

use std::{collections::HashMap, env, str::FromStr};

use rover_client::operations::subgraph::fetch::{self, SubgraphFetchInput};
use rover_client::operations::subgraph::introspect::{self, SubgraphIntrospectInput};
use rover_client::shared::GraphRef;
use rover_client::{blocking::GraphQLClient, RoverClientError};

use crate::{
    options::ProfileOpt,
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
};
use crate::{RoverError, RoverErrorSuggestion, RoverResult};

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
    let contents = unresolved_supergraph_yaml
        .read_file_descriptor("supergraph config", &mut std::io::stdin())?;
    let supergraph_config = SupergraphConfig::new_from_yaml(&contents)?;
    let maybe_specified_federation_version = supergraph_config.get_federation_version();
    let supergraph_config = supergraph_config
        .into_iter()
        .collect::<Vec<(String, SubgraphConfig)>>();

    let subgraph_definition_results: Vec<(String, RoverResult<SubgraphDefinition>)> =
        supergraph_config
            .into_par_iter()
            .map(|(subgraph_name, subgraph_data)| {
                let cloned_subgraph_name = subgraph_name.to_string();
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

                        Fs::read_file(&relative_schema_path)
                            .map_err(|e| {
                                let mut err = RoverError::new(e);
                                err.set_suggestion(RoverErrorSuggestion::ValidComposeFile);
                                err
                            })
                            .and_then(|schema| {
                                subgraph_data
                                    .routing_url
                                    .clone()
                                    .ok_or_else(err_no_routing_url)
                                    .map(|url| SubgraphDefinition::new(subgraph_name, url, &schema))
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

                                let headers = introspection_headers
                                    .clone()
                                    .map(|headers| {
                                        headers
                                            .into_iter()
                                            .map(|(k, v)| resolve_header_value(v).map(|v| (k, v)))
                                            .collect::<RoverResult<HashMap<String, String>>>()
                                    })
                                    .transpose()?
                                    .unwrap_or_default();
                                // given a federated introspection URL, use subgraph introspect to
                                // obtain SDL and add it to subgraph_definition.
                                introspect::run(SubgraphIntrospectInput { headers }, &client, false)
                                    .map(|introspection_response| {
                                        let schema = introspection_response.result;

                                        // We don't require a routing_url in config for this variant of a schema,
                                        // if one isn't provided, just use the URL they passed for introspection.
                                        let url = &subgraph_data
                                            .routing_url
                                            .clone()
                                            .unwrap_or_else(|| subgraph_url.to_string());
                                        SubgraphDefinition::new(subgraph_name, url, &schema)
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
                                        Ok(SubgraphDefinition::new(
                                            subgraph_name,
                                            url,
                                            &result.sdl.contents,
                                        ))
                                    } else {
                                        Err(err_no_routing_url())
                                    }
                                })
                            })
                    }
                    SchemaSource::Sdl { sdl } => subgraph_data
                        .routing_url
                        .clone()
                        .ok_or_else(err_no_routing_url)
                        .map(|url| SubgraphDefinition::new(subgraph_name, url, sdl)),
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
                    if let Some(suggestion) = error.suggestion() {
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
                ast::Definition::SchemaExtension(ext) => ext.directives(),
                ast::Definition::SchemaDefinition(def) => def.directives(),
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

/// If the header value is an environment variable, attempt to load it. Otherwise, return the value
/// as-is.
fn resolve_header_value(mut header_value: String) -> RoverResult<String> {
    while let Some(start_index) = header_value.rfind("${env.") {
        let end_index = match header_value
            .chars()
            .skip(start_index)
            .position(|c| c == '}')
        {
            Some(end_index) => end_index + start_index,
            None => break,
        };
        let env_var_key = match header_value.get(start_index + 6..end_index) {
            Some(env_var) => env_var,
            None => {
                return Err(RoverError::new(anyhow!(
                    "Error parsing environment variables"
                )))
            }
        };
        let env_var = match env::var(env_var_key) {
            Ok(value) => value,
            Err(_) => {
                return Err(RoverError::new(anyhow!(
                    "Environment variable {} not found.",
                    env_var_key
                )));
            }
        };
        header_value.replace_range(start_index..end_index + 1, &env_var);
    }
    Ok(header_value)
}

#[cfg(test)]
mod test_resolve_header_value {
    use super::*;

    // Env vars are global, so if you're going to reuse them you'd better make them constants
    // These point at each other for testing nested values
    const ENV_VAR_KEY_1: &str = "RESOLVE_HEADER_VALUE_TEST_VAR_1";
    const ENV_VAR_VALUE_1: &str = "RESOLVE_HEADER_VALUE_TEST_VAR_2";
    const ENV_VAR_KEY_2: &str = "RESOLVE_HEADER_VALUE_TEST_VAR_2";
    const ENV_VAR_VALUE_2: &str = "RESOLVE_HEADER_VALUE_TEST_VAR_1";

    #[test]
    fn valid_env_var() {
        let header_value = format!("${{env.{}}}", ENV_VAR_KEY_1);
        env::set_var(ENV_VAR_KEY_1, ENV_VAR_VALUE_1);
        assert_eq!(resolve_header_value(header_value).unwrap(), ENV_VAR_VALUE_1);
    }

    #[test]
    fn partial_env_var_partial_static() {
        let header_value = format!("static-part-${{env.{}}}", ENV_VAR_KEY_1);
        env::set_var(ENV_VAR_KEY_1, ENV_VAR_VALUE_1);
        assert_eq!(
            resolve_header_value(header_value).unwrap(),
            format!("static-part-{}", ENV_VAR_VALUE_1)
        );
    }

    #[test]
    fn multiple_env_vars() {
        let header_value = format!(
            "${{env.{}}}-static-part-${{env.{}}}",
            ENV_VAR_KEY_1, ENV_VAR_KEY_2
        );
        env::set_var(ENV_VAR_KEY_1, ENV_VAR_VALUE_1);
        env::set_var(ENV_VAR_KEY_2, ENV_VAR_VALUE_2);
        assert_eq!(
            resolve_header_value(header_value).unwrap(),
            format!("{}-static-part-{}", ENV_VAR_VALUE_1, ENV_VAR_VALUE_2)
        );
    }

    #[test]
    fn nested_env_vars() {
        let header_value = format!("${{env.${{env.{}}}}}", ENV_VAR_KEY_1);
        env::set_var(ENV_VAR_KEY_1, ENV_VAR_VALUE_1);
        env::set_var(ENV_VAR_KEY_2, ENV_VAR_VALUE_2);
        assert_eq!(resolve_header_value(header_value).unwrap(), ENV_VAR_VALUE_2);
    }

    #[test]
    fn not_env_var() {
        let header_value = "test_value";
        assert_eq!(
            resolve_header_value(header_value.to_string()).unwrap(),
            header_value
        );
    }

    #[test]
    fn env_var_not_found() {
        let header_value = "${env.RESOLVE_HEADER_VALUE_TEST_VAR_DOES_NOT_EXIST}";
        assert!(resolve_header_value(header_value.to_string()).is_err());
    }

    #[test]
    fn missing_end_brace() {
        let header_value = "${env.RESOLVE_HEADER_VALUE_TEST_VAR_DOES_NOT_EXIST";
        assert_eq!(
            resolve_header_value(header_value.to_string()).unwrap(),
            header_value
        );
    }

    #[test]
    fn missing_start_section() {
        let header_value = "RESOLVE_HEADER_VALUE_TEST_VAR_DOES_NOT_EXIST}";
        assert_eq!(
            resolve_header_value(header_value.to_string()).unwrap(),
            header_value
        );
    }
}
