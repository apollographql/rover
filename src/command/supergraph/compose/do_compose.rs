use std::{fs::File, io::Write, process::Command, str};

use anyhow::{anyhow, Context};
use apollo_federation_types::config::SupergraphConfig;
use apollo_federation_types::{
    config::{FederationVersion, PluginVersion},
    rover::BuildResult,
};
use camino::Utf8PathBuf;
use clap::Parser;
use regex::Regex;
use serde::Serialize;

use rover_client::RoverClientError;
use rover_std::{Emoji, Style};

use crate::command::supergraph::resolve_supergraph_yaml;
use crate::utils::{client::StudioClientConfig, parsers::FileDescriptorType};
use crate::{
    command::{
        install::{Install, Plugin},
        supergraph::compose::CompositionOutput,
    },
    options::PluginOpts,
    RoverError, RoverErrorSuggestion, RoverOutput, RoverResult,
};

#[derive(Debug, Clone, Serialize, Parser)]
pub struct Compose {
    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[serde(skip_serializing)]
    #[arg(long = "config")]
    supergraph_yaml: FileDescriptorType,

    #[clap(flatten)]
    opts: PluginOpts,
}

impl Compose {
    pub fn new(compose_opts: PluginOpts) -> Self {
        Self {
            supergraph_yaml: FileDescriptorType::File("RAM".into()),
            opts: compose_opts,
        }
    }

    pub(crate) fn maybe_install_supergraph(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        federation_version: FederationVersion,
    ) -> RoverResult<Utf8PathBuf> {
        let plugin = Plugin::Supergraph(federation_version.clone());
        if federation_version.is_fed_two() {
            self.opts
                .elv2_license_accepter
                .require_elv2_license(&client_config)?;
        }

        // and create our plugin that we may need to install from it
        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepter: self.opts.elv2_license_accepter,
        };

        // maybe do the install, maybe find a pre-existing installation, maybe fail
        let plugin_exe = install_command.get_versioned_plugin(
            override_install_path,
            client_config,
            self.opts.skip_update,
        )?;
        Ok(plugin_exe)
    }

    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        eprintln!(
            "{}resolving SDL for subgraphs defined in {}",
            Emoji::Hourglass,
            Style::Path.paint(self.supergraph_yaml.to_string())
        );
        let mut supergraph_config = resolve_supergraph_yaml(
            &self.supergraph_yaml,
            client_config.clone(),
            &self.opts.profile,
        )?;
        self.compose(override_install_path, client_config, &mut supergraph_config)
    }

    pub fn compose(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        supergraph_config: &mut SupergraphConfig,
    ) -> RoverResult<RoverOutput> {
        let output = self.exec(override_install_path, client_config, supergraph_config)?;
        Ok(RoverOutput::CompositionResult(output))
    }

    pub fn exec(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        supergraph_config: &mut SupergraphConfig,
    ) -> RoverResult<CompositionOutput> {
        // first, grab the _actual_ federation version from the config we just resolved
        // (this will always be `Some` as long as we have created with `resolve_supergraph_yaml` so it is safe to unwrap)
        let federation_version = supergraph_config.get_federation_version().unwrap();
        let exe = self.maybe_install_supergraph(
            override_install_path,
            client_config,
            federation_version.clone(),
        )?;

        // _then_, overwrite the federation_version with _only_ the major version
        // before sending it to the supergraph plugin.
        // we do this because the supergraph binaries _only_ check if the major version is correct
        // and we may want to introduce other semver things in the future.
        // this technique gives us forward _and_ backward compatibility
        // because the supergraph plugin itself only has to parse "federation_version: 1" or "federation_version: 2"
        let v = match federation_version.get_major_version() {
            0 | 1 => FederationVersion::LatestFedOne,
            2 => FederationVersion::LatestFedTwo,
            _ => unreachable!("This version of Rover does not support major versions of federation other than 1 and 2.")
        };
        supergraph_config.set_federation_version(v);
        let num_subgraphs = supergraph_config.get_subgraph_definitions()?.len();
        let supergraph_config_yaml = serde_yaml::to_string(&supergraph_config)?;
        let dir = tempfile::Builder::new().prefix("supergraph").tempdir()?;
        tracing::debug!("temp dir created at {}", dir.path().display());
        let yaml_path = Utf8PathBuf::try_from(dir.path().join("config.yml"))?;
        let mut f = File::create(&yaml_path)?;
        f.write_all(supergraph_config_yaml.as_bytes())?;
        f.sync_all()?;
        tracing::debug!("config file written to {}", &yaml_path);

        let federation_version = Self::extract_federation_version(&exe)?;

        eprintln!(
            "{}composing supergraph with Federation {}",
            Emoji::Compose,
            &federation_version
        );

        let output = Command::new(&exe)
            .args(["compose", yaml_path.as_ref()])
            .output()
            .context("Failed to execute command")?;
        let stdout = str::from_utf8(&output.stdout)
            .with_context(|| format!("Could not parse output of `{} compose`", &exe))?;

        match serde_json::from_str::<BuildResult>(stdout) {
            Ok(build_result) => match build_result {
                Ok(build_output) => Ok(CompositionOutput {
                    hints: build_output.hints,
                    supergraph_sdl: build_output.supergraph_sdl,
                    federation_version: Some(federation_version),
                }),
                Err(build_errors) => Err(RoverError::from(RoverClientError::BuildErrors {
                    source: build_errors,
                    num_subgraphs,
                })),
            },
            Err(bad_json) => Err(anyhow!("{}", bad_json))
                .with_context(|| anyhow!("{} compose output: {}", &exe, stdout))
                .with_context(|| anyhow!("Output from `{} compose` was malformed.", &exe))
                .map_err(|e| {
                    let mut error = RoverError::new(e);
                    error.set_suggestion(RoverErrorSuggestion::SubmitIssue);
                    error
                }),
        }
    }

    fn extract_federation_version(exe: &Utf8PathBuf) -> Result<String, RoverError> {
        let version_re = Regex::new(r"^.*supergraph-(v[^.]*\.[^.]*\.[^.]*)(?:\.exe)?$").unwrap();
        let captured_elements = version_re
            .captures(exe.file_name().unwrap())
            .ok_or(anyhow!(
                "No matches found for version in name of downloaded plugin"
            ))?;
        let federation_version = captured_elements
            .get(1)
            .map(|capture| capture.as_str())
            .ok_or(anyhow!(
                "Could not extract version in name of downloaded plugin"
            ))?
            .to_string();
        Ok(federation_version)
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;
    use std::fs;

    use assert_fs::TempDir;
    use rstest::rstest;
    use speculoos::assert_that;
    use speculoos::prelude::ResultAssertions;

    use houston as houston_config;
    use houston_config::Config;

    use crate::options::ProfileOpt;
    use crate::utils::client::ClientBuilder;

    use super::*;

    fn get_studio_config() -> StudioClientConfig {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        StudioClientConfig::new(
            None,
            Config::new(Some(&tmp_path), None).unwrap(),
            false,
            ClientBuilder::default(),
        )
    }

    #[rstest]
    fn it_errs_on_invalid_subgraph_path() {
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
            get_studio_config(),
            &ProfileOpt {
                profile_name: "profile".to_string()
            }
        )
        .is_err())
    }

    #[rstest]
    fn it_can_get_subgraph_definitions_from_fs() {
        let raw_good_yaml = r#"subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ./films.graphql
  people:
    routing_url: https://people.example.com
    schema:
      file: ./people.graphql"#;
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
            get_studio_config(),
            &ProfileOpt {
                profile_name: "profile".to_string()
            }
        )
        .is_ok())
    }

    #[rstest]
    fn it_can_compute_relative_schema_paths() {
        let raw_good_yaml = r#"subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ../../films.graphql
  people:
    routing_url: https://people.example.com
    schema:
        file: ../../people.graphql"#;
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
            get_studio_config(),
            &ProfileOpt {
                profile_name: "profile".to_string(),
            },
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

    #[rstest]
    #[case::simple_binary(String::from("a/b/c/d/supergraph-v2.8.5"), "v2.8.5", false)]
    #[case::simple_windows_binary(String::from("a/b/supergraph-v2.9.1.exe"), "v2.9.1", false)]
    #[case::not_supergraph(String::from("a/b/im-a-new-plugin-v2.9.1.exe"), "", true)]
    #[case::is_supergraph_but_no_version(
        String::from("a/b/supergraph/im-a-new-plugin-v2.9.1.exe"),
        "",
        true
    )]
    #[case::double_supergraph_but_no_version(String::from("a/b/supergraph/supergraph"), "", true)]
    #[case::complicated_semver(
        String::from("a/b/supergraph-v1.2.3-SNAPSHOT-123"),
        "v1.2.3-SNAPSHOT-123",
        false
    )]
    #[case::complicated_semver_windows(
        String::from("a/b/supergraph-v1.2.3-SNAPSHOT-123.exe"),
        "v1.2.3-SNAPSHOT-123",
        false
    )]
    fn it_can_extract_a_version_correctly(
        #[case] file_path: String,
        #[case] expected_value: String,
        #[case] expect_error: bool,
    ) {
        let mut fake_path = Utf8PathBuf::new();
        fake_path.push(file_path);
        let result = Compose::extract_federation_version(&fake_path);
        if expect_error {
            assert_that(&result).is_err();
        } else {
            assert_that(&result.unwrap()).is_equal_to(expected_value);
        }
    }
}
