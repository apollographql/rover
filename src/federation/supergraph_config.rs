//! Helpers for working with `supergraph.yaml` files

use std::collections::BTreeMap;
use std::env::current_dir;
use std::path;

use anyhow::anyhow;
use apollo_federation_types::config::{
    FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
};
use camino::Utf8PathBuf;
use rover_client::blocking::StudioClient;
use rover_client::operations::subgraph;
use rover_client::operations::subgraph::fetch_all::{
    SubgraphFetchAllInput, SubgraphFetchAllResponse,
};
use rover_client::shared::GraphRef;

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::utils::expansion::expand;
use crate::utils::parsers::FileDescriptorType;
use crate::RoverErrorSuggestion::InvalidSupergraphYamlSubgraphSchemaPath;
use crate::{RoverError, RoverResult};

mod resolve;

pub(crate) use resolve::{resolve_supergraph_config, ResolvedSupergraphConfig};

/// Nominal type that captures the behavior of collecting remote subgraphs into a
/// [`SupergraphConfig`] representation
#[derive(Clone, Debug)]
pub struct RemoteSubgraphs(SupergraphConfig);

impl RemoteSubgraphs {
    /// Fetches [`RemoteSubgraphs`] from Studio
    pub async fn fetch(
        client: &StudioClient,
        graph_ref: &GraphRef,
    ) -> RoverResult<RemoteSubgraphs> {
        let SubgraphFetchAllResponse {
            subgraphs,
            federation_version,
        } = subgraph::fetch_all::run(
            SubgraphFetchAllInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )
        .await?;
        let subgraphs = subgraphs
            .into_iter()
            .map(|subgraph| (subgraph.name().clone(), subgraph.into()))
            .collect();
        let supergraph_config = SupergraphConfig::new(subgraphs, federation_version);
        let remote_subgraphs = RemoteSubgraphs(supergraph_config);
        Ok(remote_subgraphs)
    }

    /// Provides a reference to the inner value of this representation
    pub fn into_inner(self) -> SupergraphConfig {
        self.0
    }
}

/// Given the combination of sources for defining a supergraph, this function:
/// 1. Fetches remote subgraphs if a graph ref is provided
/// 2. Reads in the local supergraph config if a file path is provided
/// 3. Merges the remote and local supergraph configs
pub async fn get_supergraph_config(
    graph_ref: &Option<GraphRef>,
    supergraph_config_path: Option<&FileDescriptorType>,
    federation_version: Option<&FederationVersion>,
    client_config: StudioClientConfig,
    profile_opt: &ProfileOpt,
) -> Result<Option<HybridSupergraphConfig>, RoverError> {
    // Read in Remote subgraphs
    let remote_subgraphs = match graph_ref {
        Some(graph_ref) => {
            let studio_client = client_config.get_authenticated_client(profile_opt)?;
            let remote_subgraphs = Some(RemoteSubgraphs::fetch(&studio_client, graph_ref).await?);
            eprintln!("retrieving subgraphs remotely from {}", graph_ref);
            remote_subgraphs
        }
        None => None,
    };
    let local_supergraph_config = if let Some(file_descriptor) = &supergraph_config_path {
        let config = file_descriptor
            .read_file_descriptor("supergraph config", &mut std::io::stdin())
            .and_then(|contents| expand_supergraph_yaml(&contents))?;
        // Once we have expanded the supergraph.yaml we need to make some changes to the paths
        // to ensure we maintain correct semantics
        match file_descriptor {
            FileDescriptorType::Stdin => {
                let current_dir = Utf8PathBuf::try_from(current_dir()?)?;
                Some((correctly_resolve_paths(config, &current_dir)?, None))
            }
            FileDescriptorType::File(file_path) => Some((
                correctly_resolve_paths(config, &file_path.parent().unwrap().to_path_buf())?,
                Some(file_path.clone()),
            )),
        }
    } else {
        None
    };

    // Merge Remote and Local Supergraph Configs
    let supergraph_config = merge_supergraph_configs(
        remote_subgraphs.map(|remote_subgraphs| remote_subgraphs.into_inner()),
        local_supergraph_config
            .as_ref()
            .map(|(config, _)| config.clone()),
        federation_version,
    );
    eprintln!("supergraph config loaded successfully");
    Ok(
        supergraph_config.map(|merged_config| HybridSupergraphConfig {
            file: local_supergraph_config.and_then(|(config, path)| Some((config, path?))),
            merged_config,
        }),
    )
}

#[derive(Debug)]
pub(crate) struct HybridSupergraphConfig {
    /// If part of the supergraph config came from a file on disk, this is that piece
    pub(crate) file: Option<(SupergraphConfig, Utf8PathBuf)>,
    /// The combined with remove sources
    pub(crate) merged_config: SupergraphConfig,
}

pub(crate) fn correctly_resolve_paths(
    supergraph_config: SupergraphConfig,
    root_to_resolve_from: &Utf8PathBuf,
) -> Result<SupergraphConfig, RoverError> {
    let federation_version = supergraph_config.get_federation_version();
    let subgraphs = supergraph_config
        .into_iter()
        .map(|(subgraph_name, subgraph_config)| {
            if let SchemaSource::File { file } = subgraph_config.schema {
                let potential_canonical_file = root_to_resolve_from.join(&file);
                match potential_canonical_file.canonicalize_utf8() {
                    Ok(canonical_file_name) => Ok((
                        subgraph_name,
                        SubgraphConfig {
                            routing_url: subgraph_config.routing_url,
                            schema: SchemaSource::File {
                                file: canonical_file_name,
                            },
                        },
                    )),
                    Err(err) => {
                        let mut rover_err = RoverError::new(anyhow!(err).context(format!(
                            "Could not find schema file ({}) for subgraph '{}'",
                            path::absolute(potential_canonical_file)
                                .unwrap()
                                .as_path()
                                .display(),
                            subgraph_name
                        )));
                        rover_err.set_suggestion(InvalidSupergraphYamlSubgraphSchemaPath {
                            subgraph_name,
                            supergraph_yaml_path: root_to_resolve_from.clone(),
                        });
                        Err(rover_err)
                    }
                }
            } else {
                Ok((subgraph_name, subgraph_config))
            }
        })
        .collect::<Result<BTreeMap<String, SubgraphConfig>, RoverError>>()?;
    Ok(SupergraphConfig::new(subgraphs, federation_version))
}

fn resolve_federation_version(
    requested_federation_version: Option<FederationVersion>,
    supergraph_config: &SupergraphConfig,
) -> FederationVersion {
    requested_federation_version.unwrap_or_else(|| {
        supergraph_config
            .get_federation_version()
            .unwrap_or_else(|| FederationVersion::LatestFedTwo)
    })
}

/// Expands any variables in `supergraph.yaml` files
pub(crate) fn expand_supergraph_yaml(content: &str) -> RoverResult<SupergraphConfig> {
    serde_yaml::from_str(content)
        .map_err(RoverError::from)
        .and_then(expand)
        .and_then(|v| serde_yaml::from_value(v).map_err(RoverError::from))
}

#[cfg(test)]
mod test_get_supergraph_config {
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::time::Duration;

    use anyhow::Result;
    use apollo_federation_types::config::{FederationVersion, SchemaSource, SupergraphConfig};
    use camino::Utf8PathBuf;
    use houston::Config;
    use httpmock::MockServer;
    use indoc::indoc;
    use rover_client::shared::GraphRef;
    use rstest::{fixture, rstest};
    use semver::Version;
    use serde_json::{json, Value};
    use speculoos::assert_that;
    use speculoos::prelude::OptionAssertions;
    use tempfile::{NamedTempFile, TempDir};

    use super::*;
    use crate::options::ProfileOpt;
    use crate::utils::client::{ClientBuilder, StudioClientConfig};
    use crate::utils::parsers::FileDescriptorType;

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
    fn profile_opt() -> ProfileOpt {
        ProfileOpt {
            profile_name: "profile".to_string(),
        }
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

    #[rstest]
    #[case::no_subgraphs_at_all(None, None, None)]
    #[case::only_remote_subgraphs(
        Some(String::from("products")),
        None,
        Some(vec![(String::from("products"), String::from("remote"))]),
    )]
    #[case::only_local_subgraphs(
        None,
        Some(String::from("pandas")),
        Some(vec![(String::from("pandas"), String::from("local"))]),
    )]
    #[case::both_local_and_remote_subgraphs(
        Some(String::from("products")),
        Some(String::from("pandas")),
        Some(vec![(String::from("pandas"), String::from("local")), (String::from("products"), String::from("remote"))]),
    )]
    #[case::local_takes_precedence(
        Some(String::from("pandas")),
        Some(String::from("pandas")),
        Some(vec![(String::from("pandas"), String::from("local"))]),
    )]
    #[tokio::test]
    async fn test_get_supergraph_config(
        config: Config,
        profile_opt: ProfileOpt,
        latest_fed2_version: &FederationVersion,
        #[case] remote_subgraph: Option<String>,
        #[case] local_subgraph: Option<String>,
        #[case] expected: Option<Vec<(String, String)>>,
    ) {
        let server = MockServer::start();
        let sdl = "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n"
            .to_string();
        let graphref = if let Some(name) = remote_subgraph {
            let variant = String::from("current");
            let graphref_raw = format!("{name}@{variant}");
            let url = format!("http://{}.remote.com", name);
            server.mock(|when, then| {
                let request_body_partial = json!({
                    "variables": {
                        "graph_ref": graphref_raw,
                    },
                    "operationName": "SubgraphFetchAllQuery"
                });
                let body = json!({
                  "data": {
                    "variant": {
                      "__typename": "GraphVariant",
                      "subgraphs": [
                        {
                          "name": name,
                          "url": url,
                          "activePartialSchema": {
                            "sdl": sdl
                          }
                        }
                      ]
                    }
                  }
                });
                when.method(httpmock::Method::POST)
                    .path("/")
                    .json_body_partial(request_body_partial.to_string());
                then.status(200)
                    .header("content-type", "application/json")
                    .json_body(body);
            });
            Some(GraphRef::new(name, Some(variant)).unwrap())
        } else {
            None
        };

        let studio_client_config = StudioClientConfig::new(
            Some(server.base_url()),
            config,
            false,
            ClientBuilder::default(),
            Some(Duration::from_secs(3)),
        );

        let actual_result = if let Some(name) = local_subgraph {
            let supergraph_config = format!(
                indoc! {
                    r#"
                federation_version: {}
                subgraphs:
                  {}:
                    routing_url: http://{}.local.com
                    schema:
                      sdl: "{}"
                "#
                },
                latest_fed2_version,
                name,
                name,
                sdl.escape_default()
            );
            let mut supergraph_config_path =
                NamedTempFile::new().expect("Could not create temporary file");
            supergraph_config_path
                .as_file_mut()
                .write_all(&supergraph_config.into_bytes())
                .expect("Could not write to temporary file");

            get_supergraph_config(
                &graphref,
                Some(&FileDescriptorType::File(
                    Utf8PathBuf::from_path_buf(supergraph_config_path.path().to_path_buf())
                        .unwrap(),
                )),
                Some(latest_fed2_version),
                studio_client_config,
                &profile_opt,
            )
            .await
            .expect("Could not construct SupergraphConfig")
        } else {
            get_supergraph_config(
                &graphref,
                None,
                Some(latest_fed2_version),
                studio_client_config,
                &profile_opt,
            )
            .await
            .expect("Could not construct SupergraphConfig")
        };

        if expected.is_none() {
            assert_that!(actual_result).is_none()
        } else {
            assert_that!(actual_result).is_some();
            for (idx, subgraph) in actual_result
                .unwrap()
                .merged_config
                .get_subgraph_definitions()
                .unwrap()
                .iter()
                .enumerate()
            {
                let expected_result = expected.as_ref().unwrap();
                assert_that!(subgraph.name).is_equal_to(&expected_result[idx].0);
                assert_that!(subgraph.url).is_equal_to(format!(
                    "http://{}.{}.com",
                    expected_result[idx].0, expected_result[idx].1
                ))
            }
        }
    }

    #[rstest]
    #[case::no_supplied_fed_version(None, None, FederationVersion::LatestFedTwo)]
    #[case::using_supergraph_yaml_version(
        None,
        Some(FederationVersion::LatestFedOne),
        FederationVersion::LatestFedOne
    )]
    #[case::using_requested_fed_version(
        Some(FederationVersion::LatestFedOne),
        None,
        FederationVersion::LatestFedOne
    )]
    #[case::using_requested_fed_version_with_supergraph_yaml_version(
        Some(FederationVersion::LatestFedOne),
        Some(FederationVersion::LatestFedTwo),
        FederationVersion::LatestFedOne
    )]
    fn test_resolve_federation_version(
        #[case] requested_federation_version: Option<FederationVersion>,
        #[case] supergraph_yaml_federation_version: Option<FederationVersion>,
        #[case] expected_federation_version: FederationVersion,
    ) -> Result<()> {
        let federation_version_string = supergraph_yaml_federation_version
            .map(|version| format!("federation_version: {}\n", version))
            .unwrap_or_default();
        let subgraphs = "subgraphs: {}".to_string();
        let supergraph_yaml = format!("{}{}", federation_version_string, subgraphs);
        let supergraph_config: SupergraphConfig = serde_yaml::from_str(&supergraph_yaml)?;
        let federation_version =
            resolve_federation_version(requested_federation_version, &supergraph_config);
        assert_that!(federation_version).is_equal_to(expected_federation_version);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_file_paths_become_canonicalised_on_read(
        config: Config,
        latest_fed2_version: &FederationVersion,
        profile_opt: ProfileOpt,
    ) {
        let supergraph_config = format!(
            indoc! {
                r#"
                federation_version: {}
                subgraphs:
                  my_subgraph:
                    routing_url: https://subgraphs-for-all.com/subgraph1
                    schema:
                      file: ../../../schema.graphql
                "#
            },
            latest_fed2_version,
        );
        let root_test_folder = TempDir::new().expect("Can't create top-level test folder");
        let schema_path = root_test_folder.path().join("schema.graphql");
        fs::write(schema_path.clone(), "there is something here").unwrap();

        let first_level_folder =
            TempDir::new_in(&root_test_folder).expect("Can't create first-level test folder");
        let second_level_folder =
            TempDir::new_in(&first_level_folder).expect("Can't create second-level test folder");
        let third_level_folder =
            TempDir::new_in(&second_level_folder).expect("Can't create third-level test folder");
        let supergraph_config_path = third_level_folder.path().join("supergraph.yaml");
        fs::write(
            supergraph_config_path.clone(),
            &supergraph_config.into_bytes(),
        )
        .expect("Could not write supergraph.yaml");

        let studio_client_config = StudioClientConfig::new(
            None,
            config,
            false,
            ClientBuilder::default(),
            Some(Duration::from_secs(3)),
        );

        let sc_config = get_supergraph_config(
            &None,
            Some(&FileDescriptorType::File(
                Utf8PathBuf::from_path_buf(supergraph_config_path).unwrap(),
            )),
            None,
            studio_client_config,
            &profile_opt,
        )
        .await
        .expect("Could not create Supergraph Config")
        .expect("SuperGraph Config was None which was unexpected");

        for ((_, subgraph_config), b) in sc_config
            .merged_config
            .into_iter()
            .zip([schema_path.canonicalize().unwrap()])
        {
            match subgraph_config.schema {
                SchemaSource::File { file } => {
                    assert_that!(file.as_std_path()).is_equal_to(b.as_path())
                }
                _ => panic!("Incorrect schema source found"),
            }
        }
    }
}

/// Merge local and remote supergraphs, making sure that the federation version is correct: eg, when
/// `--graph-ref` is passed, it should be the remote version; otherwise, it should be the local
/// version
fn merge_supergraph_configs(
    remote_config: Option<SupergraphConfig>,
    local_config: Option<SupergraphConfig>,
    target_federation_version: Option<&FederationVersion>,
) -> Option<SupergraphConfig> {
    match (remote_config, local_config) {
        (Some(remote_config), Some(local_config)) => {
            eprintln!("merging supergraph schema files");
            let mut merged_config = remote_config;
            merged_config.merge_subgraphs(&local_config);
            let federation_version =
                resolve_federation_version(target_federation_version.cloned(), &local_config);
            merged_config.set_federation_version(federation_version);
            Some(merged_config)
        }
        (Some(remote_config), None) => {
            let federation_version =
                resolve_federation_version(target_federation_version.cloned(), &remote_config);
            let mut merged_config = remote_config;
            merged_config.set_federation_version(federation_version);
            Some(merged_config)
        }
        (None, Some(local_config)) => {
            let federation_version =
                resolve_federation_version(target_federation_version.cloned(), &local_config);
            let mut merged_config = local_config;
            merged_config.set_federation_version(federation_version);
            Some(merged_config)
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod test_merge_supergraph_configs {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    #[once]
    fn local_supergraph_config_with_latest_fed_one_version() -> SupergraphConfig {
        let federation_version_string =
            format!("federation_version: {}\n", FederationVersion::LatestFedOne);
        let subgraphs = "subgraphs: {}".to_string();
        let supergraph_yaml = format!("{}{}", federation_version_string, subgraphs);
        let supergraph_config: SupergraphConfig = serde_yaml::from_str(&supergraph_yaml).unwrap();
        supergraph_config
    }

    #[fixture]
    #[once]
    fn supergraph_config_without_fed_version() -> SupergraphConfig {
        let supergraph_yaml = "subgraphs: {}".to_string();
        let supergraph_config: SupergraphConfig = serde_yaml::from_str(&supergraph_yaml).unwrap();
        supergraph_config
    }

    #[fixture]
    #[once]
    fn remote_supergraph_config_with_latest_fed_two_version() -> SupergraphConfig {
        let federation_version_string =
            format!("federation_version: {}\n", FederationVersion::LatestFedTwo);
        let subgraphs = "subgraphs: {}".to_string();
        let supergraph_yaml = format!("{}{}", federation_version_string, subgraphs);
        let supergraph_config: SupergraphConfig = serde_yaml::from_str(&supergraph_yaml).unwrap();
        supergraph_config
    }

    enum TestCase {
        /*
         * This block represents remote/local supergraph configs _with_ a fed version
         * */
        // When both and target, target takes precedence
        RemoteAndLocalWithTarget,
        // When both and no target, local takes precendence unless it isn't set, in which case the
        // latest fedceration version is used
        RemoteAndLocalWithoutTarget,
        // No remote, but local; target takes precendence
        NoRemoteLocalWithTarget,
        // No remote, but local; no target, local takes precedence
        NoRemoteLocalWithoutTarget,
        // Remote, no local, but with target; target takes precendence
        RemoteNoLocalWithTarget,
        // Remote, no local; no target, local takes precedence and if not present defaults to
        // latest federation version
        RemoteNoLocalWithoutTarget,
        /*
         * This block represents remote/local supergraph configs _without_ a fed version
         * */
        // Precendence goes to latest fed version
        RemoteNoFedVersionLocalNoFedVersion,
        // Precedence goes to local
        RemoteNoFedVersionLocalHasVersionNoTarget,
        // Precedence goes to remote
        RemoteFedVersionLocalNoFedVersionNoTarget,
    }

    #[rstest]
    #[case::remote_and_local_with_target(
        TestCase::RemoteAndLocalWithTarget,
        Some(FederationVersion::LatestFedOne),
        // Expected because target
        FederationVersion::LatestFedOne
    )]
    #[case::remote_and_local_without_target(
        TestCase::RemoteAndLocalWithoutTarget,
        None,
        // Expected because local has fed one
        FederationVersion::LatestFedOne
    )]
    #[case::no_remote_and_local_with_target(
        TestCase::NoRemoteLocalWithTarget,
        // Target is fed two because local has fed one
        Some(FederationVersion::LatestFedTwo),
        // Expected because target
        FederationVersion::LatestFedTwo
    )]
    #[case::no_remote_and_local_without_target(
        TestCase::NoRemoteLocalWithoutTarget,
        None,
        // Expected because local has fed one
        FederationVersion::LatestFedOne
    )]
    #[case::remote_no_local_with_target(
        TestCase::RemoteNoLocalWithTarget,
        // Tasrget is fed one because remote has fed two
        Some(FederationVersion::LatestFedOne),
        // Expected because target
        FederationVersion::LatestFedOne
    )]
    #[case::remote_no_local_without_target(
        TestCase::RemoteNoLocalWithoutTarget,
        None,
        // Expected because remote is fed two
        FederationVersion::LatestFedTwo
    )]
    #[case::remote_no_fed_local_no_fed(
        TestCase::RemoteNoFedVersionLocalNoFedVersion,
        None,
        // Expected because latest
        FederationVersion::LatestFedTwo
    )]
    #[case::remote_no_fed_local_has_version_no_target(
        TestCase::RemoteNoFedVersionLocalHasVersionNoTarget,
        None,
        // Expected because local
        FederationVersion::LatestFedOne
    )]
    #[case::remote_no_fed_local_has_version_no_target(
        TestCase::RemoteFedVersionLocalNoFedVersionNoTarget,
        None,
        // Expected because remote
        FederationVersion::LatestFedTwo
    )]
    fn it_merges_local_and_remote_supergraphs(
        #[case] test_case: TestCase,
        #[case] target_federation_version: Option<FederationVersion>,
        #[case] expected_federation_version: FederationVersion,
        local_supergraph_config_with_latest_fed_one_version: &SupergraphConfig,
        remote_supergraph_config_with_latest_fed_two_version: &SupergraphConfig,
        supergraph_config_without_fed_version: &SupergraphConfig,
    ) {
        let federation_version = match test_case {
            TestCase::RemoteAndLocalWithTarget | TestCase::RemoteAndLocalWithoutTarget => {
                merge_supergraph_configs(
                    Some(remote_supergraph_config_with_latest_fed_two_version.clone()),
                    Some(local_supergraph_config_with_latest_fed_one_version.clone()),
                    target_federation_version.as_ref(),
                )
                .unwrap()
                .get_federation_version()
                .expect("no federation version, but there should always be a federation version")
            }
            TestCase::NoRemoteLocalWithTarget | TestCase::NoRemoteLocalWithoutTarget => {
                merge_supergraph_configs(
                    None,
                    Some(local_supergraph_config_with_latest_fed_one_version.clone()),
                    target_federation_version.as_ref(),
                )
                .unwrap()
                .get_federation_version()
                .expect("no federation version, but there should always be a federation version")
            }
            TestCase::RemoteNoLocalWithTarget | TestCase::RemoteNoLocalWithoutTarget => {
                merge_supergraph_configs(
                    Some(remote_supergraph_config_with_latest_fed_two_version.clone()),
                    None,
                    target_federation_version.as_ref(),
                )
                .unwrap()
                .get_federation_version()
                .expect("no federation version, but there should always be a federation version")
            }
            TestCase::RemoteNoFedVersionLocalNoFedVersion => merge_supergraph_configs(
                Some(supergraph_config_without_fed_version.clone()),
                Some(supergraph_config_without_fed_version.clone()),
                target_federation_version.as_ref(),
            )
            .unwrap()
            .get_federation_version()
            .expect("no federation version, but there should always be a federation version"),
            TestCase::RemoteNoFedVersionLocalHasVersionNoTarget => merge_supergraph_configs(
                Some(supergraph_config_without_fed_version.clone()),
                Some(local_supergraph_config_with_latest_fed_one_version.clone()),
                target_federation_version.as_ref(),
            )
            .unwrap()
            .get_federation_version()
            .expect("no federation version, but there should always be a federation version"),
            TestCase::RemoteFedVersionLocalNoFedVersionNoTarget => merge_supergraph_configs(
                Some(remote_supergraph_config_with_latest_fed_two_version.clone()),
                Some(supergraph_config_without_fed_version.clone()),
                target_federation_version.as_ref(),
            )
            .unwrap()
            .get_federation_version()
            .expect("no federation version, but there should always be a federation version"),
        };

        assert_eq!(federation_version, expected_federation_version);
    }
}
