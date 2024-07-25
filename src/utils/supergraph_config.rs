use std::str::FromStr;

use anyhow::anyhow;
use apollo_federation_types::build::{BuildError, BuildErrors, SubgraphDefinition};
use apollo_federation_types::config::{
    FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
};
use apollo_parser::{cst, Parser};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use rover_client::blocking::GraphQLClient;
use rover_client::operations::subgraph;
use rover_client::operations::subgraph::fetch::SubgraphFetchInput;
use rover_client::operations::subgraph::introspect::SubgraphIntrospectInput;
use rover_client::operations::subgraph::list::SubgraphListInput;
use rover_client::operations::subgraph::{fetch, introspect};
use rover_client::shared::GraphRef;
use rover_client::RoverClientError;
use rover_std::{Fs, Style};

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::utils::expansion::expand;
use crate::utils::parsers::FileDescriptorType;
use crate::{RoverError, RoverErrorSuggestion, RoverResult};

/// Nominal type that captures the behavior of collecting remote subgraphs into a
/// [`SupergraphConfig`] representation
#[derive(Clone, Debug)]
pub struct RemoteSubgraphs(SupergraphConfig);

impl RemoteSubgraphs {
    /// Fetches [`RemoteSubgraphs`] from Studio
    pub fn fetch(
        client_config: &StudioClientConfig,
        profile_opt: &ProfileOpt,
        federation_version: &FederationVersion,
        graph_ref: &GraphRef,
    ) -> RoverResult<RemoteSubgraphs> {
        let client = &client_config.get_authenticated_client(profile_opt)?;

        let subgraphs = subgraph::list::run(
            SubgraphListInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )?;
        let subgraphs = subgraphs
            .subgraphs
            .iter()
            .map(|subgraph| {
                (
                    subgraph.name.clone(),
                    SubgraphConfig {
                        routing_url: subgraph.url.clone(),
                        schema: SchemaSource::Subgraph {
                            graphref: graph_ref.clone().to_string(),
                            subgraph: subgraph.name.clone(),
                        },
                    },
                )
            })
            .collect();
        let supergraph_config = SupergraphConfig::new(subgraphs, Some(federation_version.clone()));
        let remote_subgraphs = RemoteSubgraphs(supergraph_config);
        Ok(remote_subgraphs)
    }

    /// Provides a reference to the inner value of this representation
    pub fn inner(&self) -> &SupergraphConfig {
        &self.0
    }
}

pub fn get_supergraph_config(
    graph_ref: &Option<GraphRef>,
    supergraph_config_path: &Option<FileDescriptorType>,
    federation_version: &FederationVersion,
    client_config: StudioClientConfig,
    profile_opt: &ProfileOpt,
) -> Result<Option<SupergraphConfig>, RoverError> {
    // Read in Remote subgraphs
    let remote_subgraphs = match graph_ref {
        Some(graph_ref) => Some(RemoteSubgraphs::fetch(
            &client_config,
            profile_opt,
            federation_version,
            graph_ref,
        )?),
        None => None,
    };

    // Read in Local Supergraph Config
    let supergraph_config = if let Some(file_descriptor) = &supergraph_config_path {
        Some(resolve_supergraph_yaml(
            file_descriptor,
            client_config,
            profile_opt,
        )?)
    } else {
        None
    };

    // Merge Remote and Local Supergraph Configs
    let supergraph_config = match remote_subgraphs {
        Some(remote_subgraphs) => match supergraph_config {
            Some(supergraph_config) => {
                let mut merged_supergraph_config = remote_subgraphs.inner().clone();
                merged_supergraph_config.merge_subgraphs(&supergraph_config);
                Some(merged_supergraph_config)
            }
            None => Some(remote_subgraphs.inner().clone()),
        },
        None => supergraph_config,
    };
    Ok(supergraph_config)
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

                        Fs::read_file(relative_schema_path)
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
                                    let schema = introspection_response.result;

                                    // We don't require a routing_url in config for this variant of a schema,
                                    // if one isn't provided, just use the URL they passed for introspection.
                                    let url = &subgraph_data
                                        .routing_url
                                        .clone()
                                        .unwrap_or_else(|| subgraph_url.to_string());
                                    SubgraphDefinition::new(subgraph_name, url, schema)
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

fn expand_supergraph_yaml(content: &str) -> RoverResult<SupergraphConfig> {
    serde_yaml::from_str(content)
        .map_err(RoverError::from)
        .and_then(expand)
        .and_then(|v| serde_yaml::from_value(v).map_err(RoverError::from))
}

#[cfg(test)]
mod test_resolve_supergraph_yaml {
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use std::string::ToString;

    use anyhow::Result;
    use apollo_federation_types::config::{FederationVersion, SchemaSource, SubgraphConfig};
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use httpmock::MockServer;
    use indoc::indoc;
    use rstest::{fixture, rstest};
    use semver::Version;
    use serde_json::{json, Value};
    use speculoos::assert_that;
    use speculoos::prelude::{ResultAssertions, VecAssertions};

    use houston::Config;

    use crate::options::ProfileOpt;
    use crate::utils::client::{ClientBuilder, StudioClientConfig};
    use crate::utils::parsers::FileDescriptorType;

    use super::*;

    #[fixture]
    fn profile_opt() -> ProfileOpt {
        ProfileOpt {
            profile_name: "profile".to_string(),
        }
    }

    #[fixture]
    #[once]
    fn home_dir() -> Utf8PathBuf {
        tempfile::tempdir()
            .unwrap()
            .path()
            .to_path_buf()
            .try_into()
            .unwrap()
    }

    #[fixture]
    #[once]
    fn api_key() -> String {
        uuid::Uuid::new_v4().as_simple().to_string()
    }

    #[fixture]
    fn config(home_dir: &Utf8PathBuf, api_key: &String) -> Config {
        Config::new(Some(home_dir), Some(api_key.to_string())).unwrap()
    }

    #[fixture]
    fn client_config(config: Config) -> StudioClientConfig {
        StudioClientConfig::new(None, config, false, ClientBuilder::default())
    }

    #[fixture]
    #[once]
    fn latest_fed2_version() -> FederationVersion {
        let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("latest_plugin_versions.json");
        let fp = File::open(d).expect("could not open version file");
        let raw_version_file: Value = serde_json::from_reader(fp).expect("malformed JSON");
        let raw_version = raw_version_file
            .get("supergraph")
            .unwrap()
            .get("versions")
            .unwrap()
            .get("latest-2")
            .unwrap()
            .as_str()
            .unwrap();
        let version = Version::from_str(&raw_version.replace("v", "")).unwrap();
        FederationVersion::ExactFedTwo(version)
    }

    #[test]
    fn test_supergraph_yaml_int_version() {
        let yaml = indoc! {r#"
            federation_version: 1
            subgraphs:
"#
        };
        let config = super::expand_supergraph_yaml(yaml).unwrap();
        assert_eq!(
            config.get_federation_version(),
            Some(FederationVersion::LatestFedOne)
        );
    }

    #[rstest]
    fn it_errs_on_invalid_subgraph_path(
        client_config: StudioClientConfig,
        profile_opt: ProfileOpt,
    ) {
        let raw_good_yaml = r#"subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ./films-do-not-exist.graphql
  people:
    routing_url: https://people.example.com
    schema:
      file: ./people-do-not-exist.graphql"#;
        let tmp_home = TempDir::new().unwrap();
        let mut config_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        config_path.push("config.yaml");
        fs::write(&config_path, raw_good_yaml).unwrap();
        assert!(resolve_supergraph_yaml(
            &FileDescriptorType::File(config_path),
            client_config,
            &profile_opt
        )
        .is_err())
    }

    #[rstest]
    fn it_can_get_subgraph_definitions_from_fs(
        client_config: StudioClientConfig,
        profile_opt: ProfileOpt,
        latest_fed2_version: &FederationVersion,
    ) {
        let raw_good_yaml = format!(
            r#"
federation_version: {}
subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ./films.graphql
  people:
    routing_url: https://people.example.com
    schema:
      file: ./people.graphql"#,
            latest_fed2_version.to_string()
        );
        let tmp_home = TempDir::new().unwrap();
        let mut config_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        config_path.push("config.yaml");
        fs::write(&config_path, raw_good_yaml).unwrap();
        let tmp_dir = config_path.parent().unwrap().to_path_buf();
        let films_path = tmp_dir.join("films.graphql");
        let people_path = tmp_dir.join("people.graphql");
        fs::write(films_path, "there is something here").unwrap();
        fs::write(people_path, "there is also something here").unwrap();
        assert!(resolve_supergraph_yaml(
            &FileDescriptorType::File(config_path),
            client_config,
            &profile_opt
        )
        .is_ok())
    }

    #[rstest]
    fn it_can_compute_relative_schema_paths(
        client_config: StudioClientConfig,
        profile_opt: ProfileOpt,
        latest_fed2_version: &FederationVersion,
    ) {
        let raw_good_yaml = format!(
            r#"
federation_version: {}
subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ../../films.graphql
  people:
    routing_url: https://people.example.com
    schema:
        file: ../../people.graphql"#,
            latest_fed2_version.to_string()
        );
        let tmp_home = TempDir::new().unwrap();
        let tmp_dir = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let mut config_path = tmp_dir.clone();
        config_path.push("layer");
        config_path.push("layer");
        fs::create_dir_all(&config_path).unwrap();
        config_path.push("config.yaml");
        fs::write(&config_path, raw_good_yaml).unwrap();
        let films_path = tmp_dir.join("films.graphql");
        let people_path = tmp_dir.join("people.graphql");
        fs::write(films_path, "there is something here").unwrap();
        fs::write(people_path, "there is also something here").unwrap();
        let subgraph_definitions = resolve_supergraph_yaml(
            &FileDescriptorType::File(config_path),
            client_config,
            &profile_opt,
        )
        .unwrap()
        .get_subgraph_definitions()
        .unwrap();
        let film_subgraph = subgraph_definitions.first().unwrap();
        let people_subgraph = subgraph_definitions.get(1).unwrap();

        assert_eq!(film_subgraph.name, "films");
        assert_eq!(film_subgraph.url, "https://films.example.com");
        assert_eq!(film_subgraph.sdl, "there is something here");
        assert_eq!(people_subgraph.name, "people");
        assert_eq!(people_subgraph.url, "https://people.example.com");
        assert_eq!(people_subgraph.sdl, "there is also something here");
    }

    const INTROSPECTION_SDL: &str = r#"directive @key(fields: _FieldSet!, resolvable: Boolean = true) repeatable on OBJECT | INTERFACE

directive @requires(fields: _FieldSet!) on FIELD_DEFINITION

directive @provides(fields: _FieldSet!) on FIELD_DEFINITION

directive @external(reason: String) on OBJECT | FIELD_DEFINITION

directive @tag(name: String!) repeatable on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION

directive @extends on OBJECT | INTERFACE

type Query {\n  test: String!\n  _service: _Service!\n}

scalar _FieldSet

scalar _Any

type _Service {\n  sdl: String\n}"#;

    #[fixture]
    fn schema() -> String {
        String::from(indoc! {r#"
           type Query {
             test: String!
           }
    "#
        })
    }

    #[rstest]
    fn test_subgraph_file_resolution(
        schema: String,
        profile_opt: ProfileOpt,
        client_config: StudioClientConfig,
        latest_fed2_version: &FederationVersion,
    ) -> Result<()> {
        let mut schema_path = tempfile::NamedTempFile::new()?;
        schema_path
            .as_file_mut()
            .write_all(&schema.clone().into_bytes())?;
        let supergraph_config = format!(
            indoc! {r#"
          federation_version: {}
          subgraphs:
            products:
              routing_url: http://localhost:8000/
              schema:
                file: {}
"#
            },
            latest_fed2_version.to_string(),
            schema_path.path().to_str().unwrap()
        );

        let mut supergraph_config_path = tempfile::NamedTempFile::new()?;
        supergraph_config_path
            .as_file_mut()
            .write_all(&supergraph_config.into_bytes())?;

        let unresolved_supergraph_config =
            FileDescriptorType::File(supergraph_config_path.path().to_path_buf().try_into()?);

        let resolved_config = super::resolve_supergraph_yaml(
            &unresolved_supergraph_config,
            client_config,
            &profile_opt,
        );

        assert_that!(resolved_config).is_ok();
        let resolved_config = resolved_config.unwrap();

        let subgraphs = resolved_config.into_iter().collect::<Vec<_>>();
        assert_that!(subgraphs).has_length(1);
        let subgraph = &subgraphs[0];
        assert_that!(subgraph).is_equal_to(&(
            "products".to_string(),
            SubgraphConfig {
                routing_url: Some("http://localhost:8000/".to_string()),
                schema: SchemaSource::Sdl {
                    sdl: schema.to_string(),
                },
            },
        ));

        Ok(())
    }

    #[rstest]
    fn test_subgraph_introspection_resolution(
        profile_opt: ProfileOpt,
        client_config: StudioClientConfig,
        latest_fed2_version: &FederationVersion,
    ) -> Result<()> {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            let body = json!({
                "data": {
                    "_service": {
                        "sdl": INTROSPECTION_SDL
                    }
                }
            });
            when.method(httpmock::Method::POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(body);
        });

        let supergraph_config = format!(
            indoc! {r#"
          federation_version: {}
          subgraphs:
            products:
              routing_url: {}
              schema:
                subgraph_url: {}
"#
            },
            latest_fed2_version.to_string(),
            server.base_url(),
            server.base_url()
        );

        let mut supergraph_config_path = tempfile::NamedTempFile::new()?;
        supergraph_config_path
            .as_file_mut()
            .write_all(&supergraph_config.into_bytes())?;

        let unresolved_supergraph_config =
            FileDescriptorType::File(supergraph_config_path.path().to_path_buf().try_into()?);

        let resolved_config = super::resolve_supergraph_yaml(
            &unresolved_supergraph_config,
            client_config,
            &profile_opt,
        );

        mock.assert_hits(1);

        assert_that!(resolved_config).is_ok();
        let resolved_config = resolved_config.unwrap();

        let subgraphs = resolved_config.into_iter().collect::<Vec<_>>();
        assert_that!(subgraphs).has_length(1);
        let subgraph = &subgraphs[0];
        assert_that!(subgraph).is_equal_to(&(
            "products".to_string(),
            SubgraphConfig {
                routing_url: Some(server.base_url()),
                schema: SchemaSource::Sdl {
                    sdl: INTROSPECTION_SDL.to_string(),
                },
            },
        ));

        Ok(())
    }

    #[rstest]
    fn test_subgraph_studio_resolution(profile_opt: ProfileOpt, config: Config) -> Result<()> {
        let graph_id = "testgraph";
        let variant = "current";
        let graphref = format!("{}@{}", graph_id, variant);
        let server = MockServer::start();

        let subgraph_fetch_mock = server.mock(|when, then| {
            let body = json!({
              "data": {
                "variant": {
                  "__typename": "GraphVariant",
                  "subgraph": {
                    "url": server.base_url(),
                    "activePartialSchema": {
                      "sdl": INTROSPECTION_SDL
                    }
                  },
                  "subgraphs": [
                    {
                      "name": "products"
                    }
                  ]
                }
              }
            });
            when.method(httpmock::Method::POST)
                .path("/")
                .json_body_obj(&json!({
                    "query": indoc!{
                        r#"
                        query SubgraphFetchQuery($graph_ref: ID!, $subgraph_name: ID!) {
                          variant(ref: $graph_ref) {
                            __typename
                            ... on GraphVariant {
                              subgraph(name: $subgraph_name) {
                                url,
                                activePartialSchema {
                                  sdl
                                }
                              }
                              subgraphs {
                                name
                              }
                            }
                          }
                        }
                        "#
                    },
                    "variables": {
                        "graph_ref": graphref,
                        "subgraph_name": "products"
                    },
                    "operationName": "SubgraphFetchQuery"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(body);
        });

        let is_federated_mock = server.mock(|when, then| {
            let body = json!({
              "data": {
                "graph": {
                  "variant": {
                    "subgraphs": [
                      {
                        "name": "products"
                      }
                    ]
                  }
                }
              }
            });
            when.method(httpmock::Method::POST)
                .path("/")
                .json_body_obj(&json!({
                    "query": indoc!{
                      r#"
                      query IsFederatedGraph($graph_id: ID!, $variant: String!) {
                        graph(id: $graph_id) {
                          variant(name: $variant) {
                            subgraphs {
                              name
                            }
                          }
                        }
                      }
                      "#
                    },
                    "variables": {
                        "graph_id": graph_id,
                        "variant": variant
                    },
                    "operationName": "IsFederatedGraph"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(body);
        });

        let supergraph_config = format!(
            indoc! {r#"
          federation_version: 2
          subgraphs:
            products:
              schema:
                graphref: {}
                subgraph: products
"#
            },
            graphref
        );

        let studio_client_config = StudioClientConfig::new(
            Some(server.base_url()),
            config,
            false,
            ClientBuilder::default(),
        );

        let mut supergraph_config_path = tempfile::NamedTempFile::new()?;
        supergraph_config_path
            .as_file_mut()
            .write_all(&supergraph_config.into_bytes())?;

        let unresolved_supergraph_config =
            FileDescriptorType::File(supergraph_config_path.path().to_path_buf().try_into()?);

        let resolved_config = super::resolve_supergraph_yaml(
            &unresolved_supergraph_config,
            studio_client_config,
            &profile_opt,
        );

        assert_that!(resolved_config).is_ok();
        let resolved_config = resolved_config.unwrap();

        is_federated_mock.assert_hits(1);
        subgraph_fetch_mock.assert_hits(1);

        let subgraphs = resolved_config.into_iter().collect::<Vec<_>>();
        assert_that!(subgraphs).has_length(1);
        let subgraph = &subgraphs[0];
        assert_that!(subgraph).is_equal_to(&(
            "products".to_string(),
            SubgraphConfig {
                routing_url: Some(server.base_url()),
                schema: SchemaSource::Sdl {
                    sdl: INTROSPECTION_SDL.to_string(),
                },
            },
        ));

        Ok(())
    }

    #[rstest]
    fn test_subgraph_sdl_resolution(
        schema: String,
        profile_opt: ProfileOpt,
        client_config: StudioClientConfig,
        latest_fed2_version: &FederationVersion,
    ) -> Result<()> {
        let supergraph_config = format!(
            indoc! {
                r#"
                federation_version: {}
                subgraphs:
                  products:
                    routing_url: http://localhost:8000/
                    schema:
                      sdl: "{}"
                "#
            },
            latest_fed2_version.to_string(),
            schema.escape_default()
        );

        let mut supergraph_config_path = tempfile::NamedTempFile::new()?;
        supergraph_config_path
            .as_file_mut()
            .write_all(&supergraph_config.into_bytes())?;

        let unresolved_supergraph_config =
            FileDescriptorType::File(supergraph_config_path.path().to_path_buf().try_into()?);

        let resolved_config = super::resolve_supergraph_yaml(
            &unresolved_supergraph_config,
            client_config,
            &profile_opt,
        );

        assert_that!(resolved_config).is_ok();
        let resolved_config = resolved_config.unwrap();

        let subgraphs = resolved_config.into_iter().collect::<Vec<_>>();
        assert_that!(subgraphs).has_length(1);
        let subgraph = &subgraphs[0];
        assert_that!(subgraph).is_equal_to(&(
            "products".to_string(),
            SubgraphConfig {
                routing_url: Some("http://localhost:8000/".to_string()),
                schema: SchemaSource::Sdl {
                    sdl: schema.to_string(),
                },
            },
        ));

        Ok(())
    }
}
