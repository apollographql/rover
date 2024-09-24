//! The `supergraph` binary can't actually use [`SupergraphConfig`] directly, it needs this special
//! "fully resolved" version. The structs in this module should eventually be moved in to the
//! shared `apollo-federation-types` crate, but the resolution process itself belongs to Rover.

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverErrorSuggestion, RoverResult};
use anyhow::anyhow;
use apollo_federation_types::config::{
    FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
};
use apollo_federation_types::rover::{BuildError, BuildErrors};
use apollo_parser::{cst, Parser};
use futures::future::join_all;
use rover_client::blocking::GraphQLClient;
use rover_client::operations::subgraph::fetch::SubgraphFetchInput;
use rover_client::operations::subgraph::introspect::SubgraphIntrospectInput;
use rover_client::operations::subgraph::{fetch, introspect};
use rover_client::shared::GraphRef;
use rover_client::RoverClientError;
use rover_std::{Fs, Style};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;

/// The [`SupergraphConfig`] that the `supergraph` binary can actually use.
/// TODO: move this into the `apollo-federation-types` crate
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct ResolvedSupergraphConfig {
    pub(crate) subgraphs: BTreeMap<String, ResolvedSubgraphConfig>,
    pub(crate) federation_version: FederationVersion,
}

/// The [`SubgraphConfig`] that the `supergraph` binary can actually use.
/// TODO: move this into the `apollo-federation-types` crate
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct ResolvedSubgraphConfig {
    /// The routing URL for the subgraph.
    /// This will appear in supergraph SDL and
    /// instructs the graph router to send all requests
    /// for this subgraph to this URL.
    pub(crate) routing_url: String,

    /// The location of the subgraph's SDL
    pub(crate) schema: ResolvedSchemaSource,
}

/// The [`SchemaSource`] that the `supergraph` binary can actually use.
/// TODO: move this into the `apollo-federation-types` crate
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct ResolvedSchemaSource {
    pub(crate) sdl: String,
}

pub(crate) async fn resolve_supergraph_config(
    supergraph_config: SupergraphConfig,
    client_config: StudioClientConfig,
    profile_opt: &ProfileOpt,
) -> RoverResult<ResolvedSupergraphConfig> {
    let maybe_specified_federation_version = supergraph_config.get_federation_version();

    let futs = supergraph_config
        .into_iter()
        .map(|(subgraph_name, subgraph_config)| {
            resolve_subgraph(
                subgraph_name.clone(),
                subgraph_config,
                client_config.clone(),
                profile_opt,
            )
        });

    let subgraph_definition_results = join_all(futs).await.into_iter();

    let mut subgraphs = BTreeMap::new();
    let mut subgraph_config_errors = Vec::new();

    let mut fed_two_subgraph_names = Vec::new();

    for res in subgraph_definition_results {
        let (subgraph_name, subgraph_config_result) = res?;
        let (routing_url, sdl) = match subgraph_config_result {
            Ok((routing_url, sdl)) => (routing_url, sdl),
            Err(e) => {
                subgraph_config_errors.push((subgraph_name, e));
                continue;
            }
        };
        let Some(routing_url) = routing_url else {
            let err = RoverError::new(anyhow!(
                "No routing URL provided for subgraph '{}'",
                subgraph_name
            ));
            subgraph_config_errors.push((subgraph_name, err));
            continue;
        };
        subgraphs.insert(
            subgraph_name.clone(),
            ResolvedSubgraphConfig {
                routing_url,
                schema: ResolvedSchemaSource { sdl: sdl.clone() },
            },
        );
        let parser = Parser::new(&sdl);
        let parsed_ast = parser.parse();
        let doc = parsed_ast.document();
        'definitions: for definition in doc.definitions() {
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
                            fed_two_subgraph_names.push(subgraph_name);
                            break 'definitions;
                        }
                    }
                }
            }
        }
    }

    if !subgraph_config_errors.is_empty() {
        let source = BuildErrors::from(
            subgraph_config_errors
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
        return Err(RoverError::from(RoverClientError::BuildErrors { source }));
    }

    let print_inexact_warning = || {
        eprintln!("{} An exact {} was not specified in supergraph config. Future versions of Rover will fail when an exact federation version is not specified. See {} for more information.", Style::WarningPrefix.paint("WARN:"), Style::Command.paint("federation_version"), Style::Link.paint("https://www.apollographql.com/docs/rover/commands/supergraphs#setting-a-composition-version"))
    };

    let federation_version = if let Some(specified_federation_version) =
        maybe_specified_federation_version
    {
        // error if we detect an `@link` directive and the explicitly set `federation_version` to 1
        if specified_federation_version.is_fed_one() && !fed_two_subgraph_names.is_empty() {
            let mut err =
                RoverError::new(anyhow!("The 'federation_version' set in the supergraph config is invalid. The following subgraphs contain '@link' directives, which are only valid in Federation 2: {}", fed_two_subgraph_names.join(", ")));
            err.set_suggestion(RoverErrorSuggestion::Adhoc(String::from(
                "Either remove the 'federation_version' entry from the supergraph config, or set the value to '2'.",
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
        specified_federation_version
    } else if fed_two_subgraph_names.is_empty() {
        // if they did not specify a version and no subgraphs contain `@link` directives, use Federation 1
        print_inexact_warning();
        FederationVersion::LatestFedOne
    } else {
        // if they did not specify a version and at least one subgraph contains an `@link` directive, use Federation 2
        print_inexact_warning();
        FederationVersion::LatestFedTwo
    };

    Ok(ResolvedSupergraphConfig {
        federation_version,
        subgraphs,
    })
}

pub(crate) async fn resolve_subgraph(
    subgraph_name: String,
    subgraph_data: SubgraphConfig,
    client_config: StudioClientConfig,
    profile_opt: &ProfileOpt,
) -> RoverResult<(String, RoverResult<(Option<String>, String)>)> {
    let cloned_subgraph_name = subgraph_name.to_string();
    let result = match &subgraph_data.schema {
        SchemaSource::File { file } => Fs::read_file(file)
            .map_err(|e| {
                let mut err = RoverError::new(e);
                err.set_suggestion(RoverErrorSuggestion::ValidComposeFile);
                err
            })
            .map(|schema| (subgraph_data.routing_url.clone(), schema)),
        SchemaSource::SubgraphIntrospection {
            subgraph_url,
            introspection_headers,
        } => {
            let client = client_config
                .get_reqwest_client()
                .map_err(RoverError::from)?;
            let client =
                GraphQLClient::new(subgraph_url.as_ref(), client, client_config.retry_period);

            // given a federated introspection URL, use subgraph introspect to
            // obtain SDL and add it to subgraph_definition.
            introspect::run(
                SubgraphIntrospectInput {
                    headers: introspection_headers.clone().unwrap_or_default(),
                },
                &client,
                false,
            )
            .await
            .map(|introspection_response| {
                let schema = introspection_response.result;

                (
                    // We don't require a routing_url in config for
                    // this variant of a schema, if one isn't
                    // provided, just use the URL they passed for
                    // introspection. (This does mean there's no way
                    // when combining `--graph-ref` and a config
                    // file to say "fetch the schema from
                    // introspection but use the routing URL from
                    // the graph" at the moment.)
                    subgraph_data
                        .routing_url
                        .clone()
                        .or_else(|| Some(subgraph_url.to_string())),
                    schema,
                )
            })
            .map_err(RoverError::from)
        }
        SchemaSource::Subgraph {
            graphref: graph_ref,
            subgraph,
        } => {
            // WARNING: here's where we're returning an error on invalid graph refs; before
            // this would bubble up and, I _think_, early abort the resolving
            let graph_ref = match GraphRef::from_str(graph_ref) {
                Ok(graph_ref) => graph_ref,
                Err(_err) => {
                    return {
                        let err = anyhow!("Invalid graph ref.");
                        let mut err = RoverError::new(err);
                        err.set_suggestion(RoverErrorSuggestion::CheckGraphNameAndAuth);
                        Err(err)
                    }
                }
            };

            let authenticated_client = client_config
                .get_authenticated_client(profile_opt)
                .map_err(RoverError::from)?;

            //let graph_ref = GraphRef::from_str(graph_ref).unwrap();
            // given a graph_ref and subgraph, run subgraph fetch to
            // obtain SDL and add it to subgraph_definition.
            fetch::run(
                SubgraphFetchInput {
                    graph_ref,
                    subgraph_name: subgraph.clone(),
                },
                &authenticated_client,
            )
            .await
            .map_err(RoverError::from)
            .map(|result| {
                // We don't require a routing_url in config for this variant of a schema,
                // if one isn't provided, just use the routing URL from the graph registry (if it exists).
                if let rover_client::shared::SdlType::Subgraph {
                    routing_url: Some(graph_registry_routing_url),
                } = result.sdl.r#type
                {
                    (
                        subgraph_data
                            .routing_url
                            .clone()
                            .or(Some(graph_registry_routing_url)),
                        result.sdl.contents,
                    )
                } else {
                    panic!("whoops: rebase me");
                }
            })
        }
        SchemaSource::Sdl { sdl } => Ok((subgraph_data.routing_url.clone(), sdl.clone())),
    };
    Ok((cloned_subgraph_name, result))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::File;
    use std::path::PathBuf;
    use std::string::ToString;
    use std::time::Duration;

    use anyhow::Result;
    use apollo_federation_types::config::{FederationVersion, SubgraphConfig};
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use httpmock::MockServer;
    use indoc::indoc;
    use rstest::{fixture, rstest};
    use semver::Version;
    use serde_json::{json, Value};
    use speculoos::assert_that;
    use speculoos::prelude::ResultAssertions;

    use super::*;
    use crate::federation::supergraph_config::expand_supergraph_yaml;
    use crate::options::ProfileOpt;
    use crate::utils::client::{ClientBuilder, StudioClientConfig};
    use houston::Config;

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
        StudioClientConfig::new(
            None,
            config,
            false,
            ClientBuilder::default(),
            Some(Duration::from_secs(3)),
        )
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
        let config = expand_supergraph_yaml(yaml).unwrap();
        assert_eq!(
            config.get_federation_version(),
            Some(FederationVersion::LatestFedOne)
        );
    }

    #[rstest]
    #[tokio::test]
    async fn it_errs_on_invalid_subgraph_path(
        client_config: StudioClientConfig,
        profile_opt: ProfileOpt,
    ) {
        let supergraph_config = SupergraphConfig::new_from_yaml(
            r#"subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ./films-do-not-exist.graphql
  people:
    routing_url: https://people.example.com
    schema:
      file: ./people-do-not-exist.graphql"#,
        )
        .unwrap();
        assert!(
            resolve_supergraph_config(supergraph_config, client_config, &profile_opt,)
                .await
                .is_err()
        )
    }

    #[rstest]
    #[tokio::test]
    async fn it_can_get_subgraph_definitions_from_fs(
        client_config: StudioClientConfig,
        profile_opt: ProfileOpt,
        latest_fed2_version: &FederationVersion,
    ) {
        let tmp_home = TempDir::new().unwrap();
        let config_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let tmp_dir = config_path.parent().unwrap().to_path_buf();
        let films_path = tmp_dir.join("films.graphql");
        let people_path = tmp_dir.join("people.graphql");

        let subgraphs = BTreeMap::from_iter([
            (
                "films".to_string(),
                SubgraphConfig {
                    routing_url: Some("https://films.example.com".to_string()),
                    schema: SchemaSource::File {
                        file: films_path.clone(),
                    },
                },
            ),
            (
                "people".to_string(),
                SubgraphConfig {
                    routing_url: Some("https://people.example.com".to_string()),
                    schema: SchemaSource::File {
                        file: people_path.clone(),
                    },
                },
            ),
        ]);
        let supergraph_config = SupergraphConfig::new(subgraphs, Some(latest_fed2_version.clone()));

        fs::write(films_path, "there is something here").unwrap();
        fs::write(people_path, "there is also something here").unwrap();
        assert!(
            resolve_supergraph_config(supergraph_config, client_config, &profile_opt,)
                .await
                .is_ok()
        )
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
    #[tokio::test]
    async fn test_subgraph_introspection_resolution(
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

        let supergraph_config = SupergraphConfig::new_from_yaml(&format!(
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
        ))
        .unwrap();

        let resolved_config =
            resolve_supergraph_config(supergraph_config, client_config, &profile_opt).await;

        mock.assert_hits(1);

        assert_that!(resolved_config).is_ok();
        let resolved_config = resolved_config.unwrap();

        let mut subgraphs = resolved_config.subgraphs;
        assert_eq!(subgraphs.len(), 1);
        let subgraph = subgraphs.pop_first().unwrap();
        assert_that!(subgraph).is_equal_to((
            "products".to_string(),
            ResolvedSubgraphConfig {
                routing_url: server.base_url(),
                schema: ResolvedSchemaSource {
                    sdl: INTROSPECTION_SDL.to_string(),
                },
            },
        ));

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_subgraph_studio_resolution(
        profile_opt: ProfileOpt,
        config: Config,
        latest_fed2_version: &FederationVersion,
    ) -> Result<()> {
        let graph_id = "testgraph";
        let variant = "current";
        let graphref = format!("{}@{}", graph_id, variant);
        let server = MockServer::start_async().await;

        let subgraph_fetch_mock = server.mock(|when, then| {
            let request_partial = json!({
                "variables": {
                    "graph_ref": graphref,
                    "subgraph_name": "products"
                },
                "operationName": "SubgraphFetchQuery"
            });
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
                .json_body_partial(request_partial.to_string());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(body);
        });

        let supergraph_config = SupergraphConfig::new_from_yaml(&format!(
            indoc! {r#"
          federation_version: {}
          subgraphs:
            products:
              schema:
                graphref: {}
                subgraph: products
"#
            },
            latest_fed2_version, graphref
        ))
        .unwrap();

        let studio_client_config = StudioClientConfig::new(
            Some(server.base_url()),
            config,
            false,
            ClientBuilder::default(),
            Some(Duration::from_secs(3)),
        );

        let resolved_config =
            resolve_supergraph_config(supergraph_config, studio_client_config, &profile_opt).await;

        assert_that!(resolved_config).is_ok();
        let resolved_config = resolved_config.unwrap();

        subgraph_fetch_mock.assert_hits(1);

        let mut subgraphs = resolved_config.subgraphs;
        assert_eq!(subgraphs.len(), 1);
        let subgraph = subgraphs.pop_first().unwrap();
        assert_that!(subgraph).is_equal_to(&(
            "products".to_string(),
            ResolvedSubgraphConfig {
                routing_url: server.base_url(),
                schema: ResolvedSchemaSource {
                    sdl: INTROSPECTION_SDL.to_string(),
                },
            },
        ));

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_subgraph_sdl_resolution(
        schema: String,
        profile_opt: ProfileOpt,
        client_config: StudioClientConfig,
        latest_fed2_version: &FederationVersion,
    ) -> Result<()> {
        let supergraph_config = SupergraphConfig::new_from_yaml(&format!(
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
        ))
        .unwrap();

        let resolved_config =
            resolve_supergraph_config(supergraph_config, client_config, &profile_opt).await;

        assert_that!(resolved_config).is_ok();
        let resolved_config = resolved_config.unwrap();

        let mut subgraphs = resolved_config.subgraphs;
        assert_eq!(subgraphs.len(), 1);
        let subgraph = subgraphs.pop_first().unwrap();
        assert_that!(subgraph).is_equal_to(&(
            "products".to_string(),
            ResolvedSubgraphConfig {
                routing_url: "http://localhost:8000/".to_string(),
                schema: ResolvedSchemaSource {
                    sdl: schema.to_string(),
                },
            },
        ));

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_subgraph_federation_version_default(
        profile_opt: ProfileOpt,
        client_config: StudioClientConfig,
    ) -> Result<()> {
        let resolved_config = resolve_supergraph_config(
            SupergraphConfig::new(BTreeMap::new(), None),
            client_config,
            &profile_opt,
        )
        .await;

        assert_that!(resolved_config).is_ok();
        let resolved_config = resolved_config.unwrap();
        assert_eq!(
            resolved_config.federation_version,
            FederationVersion::LatestFedOne
        );

        Ok(())
    }
}
