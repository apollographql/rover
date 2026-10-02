//! `rover.yaml`: a user-authored declaration of which plugins, at which
//! version requests, a machine or a project expects.
//!
//! The manifest is named for Rover rather than for plugins so that it can
//! carry other configuration later. That is why the two levels of the file
//! treat unknown keys differently: an unrecognized top-level key is ignored,
//! so a manifest written for a newer Rover does not break an older one, while
//! an unrecognized key inside `plugins:` is an error, since there are exactly
//! three plugins and a misspelled one would otherwise vanish without a trace.

use std::{borrow::Cow, collections::BTreeMap, fmt};

use camino::Utf8PathBuf;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};

use super::version::{ACCEPTED_FORMS, PluginName, VersionRequest};

mod load;

pub use load::{MANIFEST_FILE, ManifestSettings};

/// The contents of one `rover.yaml`, at either the global or the project level.
///
/// Unknown top-level keys are ignored rather than rejected; see the module
/// documentation for why.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(expecting = "a mapping of manifest keys such as `plugins`")]
#[schemars(
    description = "The contents of one `rover.yaml`, at either the global or the project level. Unknown top-level keys are ignored."
)]
pub struct RoverManifest {
    /// The plugins this level declares, each with the version it asks for.
    #[serde(default)]
    pub plugins: PluginDeclarations,

    /// Where this level's plugin binaries live, in place of the directory
    /// holding this manifest. A relative path resolves against that directory.
    /// Not supported yet.
    //
    // A recognized key, so that the ignore-unknown-keys rule cannot swallow
    // it: until it is honored, a manifest that sets it has to be refused
    // rather than have its binaries land somewhere it did not ask for.
    #[serde(default)]
    #[schemars(
        with = "Option<String>",
        description = "Where this level's plugin binaries live, in place of the directory holding this manifest. A relative path resolves against that directory. Not supported yet."
    )]
    pub install_root: Option<Utf8PathBuf>,

    // Settings for every command run in this project, read by settings
    // resolution through `RoverManifest::load_settings`, never by the plugin
    // system: the two sections' rules stay independent, so the plugin side
    // neither validates nor refuses anything under `settings:`. Declared at
    // all only so the JSON schema documents the section.
    #[serde(default)]
    #[schemars(
        description = "Settings for every command run in this project. Not read by the plugin system."
    )]
    settings: SettingsSection,

    // serde_yaml does not expand YAML merge keys; it hands `<<` over as an
    // ordinary key, which the ignore-unknown-keys rule would then drop along
    // with every declaration merged through it. Refuse it instead.
    #[serde(rename = "<<", default, deserialize_with = "no_merge_keys")]
    #[schemars(skip)]
    merge_key: (),
}

fn no_merge_keys<'de, D: Deserializer<'de>>(_deserializer: D) -> Result<(), D::Error> {
    Err(de::Error::custom(
        "YAML merge keys (`<<`) aren't supported in a manifest",
    ))
}

/// The `settings:` section, as the plugin side sees it: anything at all,
/// ignored.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SettingsSection;

impl<'de> Deserialize<'de> for SettingsSection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        de::IgnoredAny::deserialize(deserializer).map(|_| Self)
    }
}

impl JsonSchema for SettingsSection {
    fn schema_name() -> Cow<'static, str> {
        "SettingsSection".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": ["object", "null"],
            "description": "Settings for every command run in this project, keyed by the setting's environment variable name (`APOLLO_REGISTRY_URL`) or its all-lowercase form (`apollo_registry_url`). A flag, an environment variable, or an explicitly selected profile overrides a value set here. Credentials can't be set here.",
            "additionalProperties": { "type": ["string", "integer", "boolean"] }
        })
    }
}

/// One plugin's entry under `plugins:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDeclaration {
    /// The version the entry asks for.
    pub request: VersionRequest,
    /// The version exactly as it was written, which [`Self::request`] cannot
    /// recover once a legacy spelling has been parsed into its modern form.
    pub written: String,
}

/// The `plugins:` section: at most one version request per plugin, keyed by a
/// name that must be one of the three plugins.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PluginDeclarations(BTreeMap<PluginName, PluginDeclaration>);

impl PluginDeclarations {
    pub fn get(&self, plugin: PluginName) -> Option<&PluginDeclaration> {
        self.0.get(&plugin)
    }

    /// Every declaration, in the order [`PluginName::ALL`] names the plugins.
    pub fn iter(&self) -> impl Iterator<Item = (PluginName, &PluginDeclaration)> {
        PluginName::ALL
            .into_iter()
            .filter_map(|plugin| self.get(plugin).map(|declaration| (plugin, declaration)))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> Deserialize<'de> for PluginDeclarations {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // `plugins:` with nothing after it, or with an explicit null,
        // declares nothing, the same as leaving the key out.
        Option::<PluginDeclarationsMap>::deserialize(deserializer)
            .map(|declarations| declarations.map(|map| map.0).unwrap_or_default())
    }
}

struct PluginDeclarationsMap(PluginDeclarations);

impl<'de> Deserialize<'de> for PluginDeclarationsMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_map(PluginDeclarationsVisitor)
            .map(Self)
    }
}

struct PluginDeclarationsVisitor;

impl<'de> Visitor<'de> for PluginDeclarationsVisitor {
    type Value = PluginDeclarations;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a mapping of plugin name to version")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut declarations = BTreeMap::new();

        while let Some(name) = map.next_key::<String>()? {
            let plugin: PluginName = name.parse().map_err(clause)?;

            // serde_yaml keeps the last of two identical keys without saying
            // so, which would let one of two conflicting requests win silently.
            if declarations.contains_key(&plugin) {
                return Err(de::Error::custom(format!(
                    "`{plugin}` is declared more than once"
                )));
            }

            let written: String = map.next_value::<Option<String>>()?.ok_or_else(|| {
                clause(format!(
                    "`{plugin}` has no version. Accepted forms are {ACCEPTED_FORMS}."
                ))
            })?;
            let request = VersionRequest::parse_for(plugin, &written).map_err(clause)?;
            declarations.insert(plugin, PluginDeclaration { request, written });
        }

        Ok(PluginDeclarations(declarations))
    }
}

/// A deserialization error from a message written as a sentence. serde_yaml
/// follows the message with its location, so a closing period would land in
/// the middle: "... `apollo-mcp-server`. at line 2 column 3".
fn clause<E: de::Error>(message: impl fmt::Display) -> E {
    E::custom(message.to_string().trim_end_matches('.'))
}

impl JsonSchema for PluginDeclarations {
    fn schema_name() -> Cow<'static, str> {
        "PluginDeclarations".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        // An unquoted bare major is a YAML integer, and Rover accepts it.
        let version = json_schema!({
            "type": ["string", "integer"],
            "description": format!("Accepted forms are {ACCEPTED_FORMS}."),
        });
        let properties: serde_json::Map<String, serde_json::Value> = PluginName::ALL
            .into_iter()
            .map(|plugin| (plugin.to_string(), version.clone().into()))
            .collect();

        json_schema!({
            "type": ["object", "null"],
            "properties": properties,
            "additionalProperties": false
        })
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use insta::assert_json_snapshot;
    use rstest::rstest;
    use schemars::schema_for;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;

    fn declarations(entries: &[(PluginName, VersionRequest, &str)]) -> PluginDeclarations {
        PluginDeclarations(
            entries
                .iter()
                .map(|(plugin, request, written)| {
                    (
                        *plugin,
                        PluginDeclaration {
                            request: request.clone(),
                            written: written.to_string(),
                        },
                    )
                })
                .collect(),
        )
    }

    #[rstest]
    #[case::every_plugin_in_every_form(
        indoc! {r#"
            plugins:
              supergraph: "2"
              router: "=2.1.0"
              apollo-mcp-server: latest
        "#},
        RoverManifest {
            plugins: declarations(&[
                (PluginName::Supergraph, VersionRequest::Major(2), "2"),
                (PluginName::Router, VersionRequest::Exact(Version::new(2, 1, 0)), "=2.1.0"),
                (PluginName::ApolloMcpServer, VersionRequest::Latest, "latest"),
            ]),
            ..RoverManifest::default()
        }
    )]
    // YAML reads an unquoted `2` as an integer, but the text is what a user
    // wrote and what the grammar parses.
    #[case::an_unquoted_major(
        indoc! {r#"
            plugins:
              supergraph: 2
        "#},
        RoverManifest {
            plugins: declarations(&[(PluginName::Supergraph, VersionRequest::Major(2), "2")]),
            ..RoverManifest::default()
        }
    )]
    #[case::a_legacy_spelling_parses_as_its_modern_form(
        indoc! {r#"
            plugins:
              supergraph: latest-2
        "#},
        RoverManifest {
            plugins: declarations(&[(PluginName::Supergraph, VersionRequest::Major(2), "latest-2")]),
            ..RoverManifest::default()
        }
    )]
    #[case::one_plugin(
        indoc! {r#"
            plugins:
              router: "=2.1.0"
        "#},
        RoverManifest {
            plugins: declarations(&[
                (PluginName::Router, VersionRequest::Exact(Version::new(2, 1, 0)), "=2.1.0"),
            ]),
            ..RoverManifest::default()
        }
    )]
    #[case::an_empty_file("", RoverManifest::default())]
    #[case::a_plugins_key_with_nothing_under_it("plugins:\n", RoverManifest::default())]
    #[case::an_empty_plugins_mapping("plugins: {}\n", RoverManifest::default())]
    #[case::a_null_plugins_key("plugins: ~\n", RoverManifest::default())]
    #[case::a_spelled_out_null_plugins_key("plugins: null\n", RoverManifest::default())]
    #[case::an_unrecognized_top_level_key_is_ignored(
        indoc! {r#"
            allow_automatic_download: true
            something_newer:
              nested: [1, 2]
            plugins:
              supergraph: "2"
        "#},
        RoverManifest {
            plugins: declarations(&[(PluginName::Supergraph, VersionRequest::Major(2), "2")]),
            ..RoverManifest::default()
        }
    )]
    #[case::install_root_is_a_recognized_key(
        "install_root: ../vendor/rover\n",
        RoverManifest {
            install_root: Some(Utf8PathBuf::from("../vendor/rover")),
            ..RoverManifest::default()
        }
    )]
    fn a_valid_manifest_parses(#[case] input: &str, #[case] expected: RoverManifest) {
        assert_that!(serde_yaml::from_str::<RoverManifest>(input))
            .is_ok()
            .is_equal_to(expected);
    }

    #[rstest]
    #[case::an_unknown_plugin_name(
        indoc! {r#"
            plugins:
              supergraph: "2"
              apollo-router: latest
        "#},
        "plugins: `apollo-router` is not a Rover plugin. Valid plugins are `supergraph`, \
         `router`, and `apollo-mcp-server` at line 2 column 3"
    )]
    #[case::an_unparseable_version(
        indoc! {r#"
            plugins:
              router: "2.1.0"
        "#},
        "plugins: `2.1.0` is not a valid version for the `router` plugin. Accepted forms are \
         `latest`, a major version such as `2`, or an exact version such as `=2.9.0` at line 2 \
         column 3"
    )]
    #[case::a_plugin_declared_twice(
        indoc! {r#"
            plugins:
              router: "2"
              router: "=2.1.0"
        "#},
        "plugins: `router` is declared more than once at line 2 column 3"
    )]
    #[case::a_plugin_declared_twice_is_reported_before_its_second_version(
        indoc! {r#"
            plugins:
              router: "2"
              router: "2.1.0"
        "#},
        "plugins: `router` is declared more than once at line 2 column 3"
    )]
    #[case::a_plugin_with_nothing_after_it(
        indoc! {r#"
            plugins:
              router:
        "#},
        "plugins: `router` has no version. Accepted forms are `latest`, a major version such as \
         `2`, or an exact version such as `=2.9.0` at line 2 column 3"
    )]
    #[case::a_plugin_with_a_null_version(
        indoc! {r#"
            plugins:
              router: ~
        "#},
        "plugins: `router` has no version. Accepted forms are `latest`, a major version such as \
         `2`, or an exact version such as `=2.9.0` at line 2 column 3"
    )]
    #[case::a_version_that_is_not_a_string(
        indoc! {r#"
            plugins:
              router: [2]
        "#},
        "plugins.router: invalid type: sequence, expected a string at line 2 column 11"
    )]
    #[case::plugins_that_is_not_a_mapping(
        indoc! {r#"
            plugins:
              - supergraph
        "#},
        "plugins: invalid type: sequence, expected a mapping of plugin name to version at line 2 \
         column 3"
    )]
    #[case::a_top_level_that_is_not_a_mapping(
        "- plugins\n",
        "invalid type: sequence, expected a mapping of manifest keys such as `plugins`"
    )]
    #[case::a_merge_key(
        indoc! {r#"
            defaults: &d
              install_root: ../vendor
            <<: *d
        "#},
        "YAML merge keys (`<<`) aren't supported in a manifest"
    )]
    #[case::not_yaml(
        indoc! {r#"
            plugins:
              supergraph: "2
        "#},
        "found unexpected end of stream at line 3 column 1, while scanning a quoted scalar at line 2 \
         column 15"
    )]
    fn an_invalid_manifest_is_rejected_saying_why(#[case] input: &str, #[case] expected: &str) {
        let error = serde_yaml::from_str::<RoverManifest>(input).expect_err("should not parse");

        assert_that!(error.to_string().as_str()).is_equal_to(expected);
    }

    /// Whatever `settings:` holds - even a credential, or something that
    /// isn't a mapping at all - is settings resolution's to judge, not the
    /// plugin system's.
    #[rstest]
    #[case::settings_rover_refuses("settings:\n  APOLLO_KEY: x\nplugins:\n  router: latest\n")]
    #[case::settings_that_arent_a_mapping("settings: [a, b]\nplugins:\n  router: latest\n")]
    fn the_plugin_side_ignores_the_settings_section(#[case] input: &str) {
        let manifest: RoverManifest = serde_yaml::from_str(input).unwrap();

        assert_that!(manifest).is_equal_to(RoverManifest {
            plugins: declarations(&[(PluginName::Router, VersionRequest::Latest, "latest")]),
            ..RoverManifest::default()
        });
    }

    #[rstest]
    fn declarations_iterate_in_plugin_order() {
        let manifest: RoverManifest = serde_yaml::from_str(indoc! {r#"
            plugins:
              apollo-mcp-server: latest
              router: latest
              supergraph: latest
        "#})
        .unwrap();

        assert_that!(
            manifest
                .plugins
                .iter()
                .map(|(plugin, _)| plugin)
                .collect::<Vec<_>>()
        )
        .is_equal_to(PluginName::ALL.to_vec());
    }

    #[rstest]
    fn the_json_schema_names_the_three_plugins_and_documents_settings() {
        assert_json_snapshot!(schema_for!(RoverManifest));
    }
}
