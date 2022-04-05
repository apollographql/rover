use crate::command::supergraph::resolve_supergraph_yaml;
use crate::utils::client::StudioClientConfig;
use crate::{
    anyhow,
    command::{
        install::{license_accept, Install, Plugin},
        RoverOutput,
    },
    error::{RoverError, Suggestion},
    Context, Result,
};

use apollo_federation_types::{build::BuildResult, config::FederationVersion};
use rover_client::RoverClientError;

use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;
use tempdir::TempDir;

use std::{fs::File, io::Write, process::Command, str};

#[derive(Debug, Serialize, StructOpt)]
pub struct Compose {
    /// The relative path to the supergraph configuration file.
    #[structopt(long = "config")]
    #[serde(skip_serializing)]
    config_path: Utf8PathBuf,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// Accept the elv2 license if you are using federation 2.
    /// Note that you only need to do this once per machine.
    #[structopt(long = "elv2-license", parse(from_str = license_accept), case_insensitive = true, env = "APOLLO_ELV2_LICENSE")]
    elv2_license_accepted: Option<bool>,

    /// Skip the update check
    #[structopt(long = "skip-update")]
    skip_update: bool,
}

impl Compose {
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        let mut supergraph_config =
            resolve_supergraph_yaml(&self.config_path, client_config.clone(), &self.profile_name)?;
        // first, grab the _actual_ federation version from the config we just resolved
        let federation_version = supergraph_config.get_federation_version();
        // and create our plugin that we may need to install from it
        let plugin = Plugin::Supergraph(federation_version.clone());
        let plugin_name = plugin.get_name();
        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepted: self.elv2_license_accepted,
        };

        // maybe do the install, maybe find a pre-existing installation, maybe fail
        let exe = install_command.get_versioned_plugin(
            override_install_path,
            client_config,
            self.skip_update,
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
        let supergraph_config_yaml = serde_yaml::to_string(&supergraph_config)?;
        let dir = TempDir::new(&plugin_name)?;
        tracing::debug!("temp dir created at {}", dir.path().display());
        let yaml_path = Utf8PathBuf::try_from(dir.path().join("config.yml"))?;
        let mut f = File::create(&yaml_path)?;
        f.write_all(supergraph_config_yaml.as_bytes())?;
        f.sync_all()?;
        tracing::debug!("config file written to {}", &yaml_path);
        let output = Command::new(&exe)
            .args(&["compose", &yaml_path.to_string()])
            .output()
            .context("Failed to execute command")?;
        let stdout = str::from_utf8(&output.stdout)
            .with_context(|| format!("Could not parse output of `{} compose`", &exe))?;

        match serde_json::from_str::<BuildResult>(stdout) {
            Ok(build_result) => match build_result {
                Ok(build_output) => Ok(RoverOutput::CompositionResult {
                    hints: build_output.hints,
                    supergraph_sdl: build_output.supergraph_sdl,
                }),
                Err(build_errors) => Err(RoverError::from(RoverClientError::BuildErrors {
                    source: build_errors,
                })),
            },
            Err(bad_json) => Err(anyhow!("{}", bad_json))
                .with_context(|| anyhow!("{} compose output: {}", &exe, stdout))
                .with_context(|| anyhow!("Output from `{} compose` was malformed.", &exe))
                .map_err(|e| {
                    let mut error = RoverError::new(e);
                    error.set_suggestion(Suggestion::SubmitIssue);
                    error
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use houston as houston_config;
    use houston_config::Config;
    use reqwest::blocking::Client;
    use std::convert::TryFrom;
    use std::fs;

    fn get_studio_config() -> StudioClientConfig {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        StudioClientConfig::new(
            None,
            Config::new(Some(&tmp_path), None).unwrap(),
            false,
            Client::new(),
        )
    }

    #[test]
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
        assert!(resolve_supergraph_yaml(&config_path, get_studio_config(), "profile").is_err())
    }

    #[test]
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
        assert!(resolve_supergraph_yaml(&config_path, get_studio_config(), "profile").is_ok())
    }

    #[test]
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
        let subgraph_definitions =
            resolve_supergraph_yaml(&config_path, get_studio_config(), "profile")
                .unwrap()
                .get_subgraph_definitions()
                .unwrap();
        let film_subgraph = subgraph_definitions.get(0).unwrap();
        let people_subgraph = subgraph_definitions.get(1).unwrap();

        assert_eq!(film_subgraph.name, "films");
        assert_eq!(film_subgraph.url, "https://films.example.com");
        assert_eq!(film_subgraph.sdl, "there is something here");
        assert_eq!(people_subgraph.name, "people");
        assert_eq!(people_subgraph.url, "https://people.example.com");
        assert_eq!(people_subgraph.sdl, "there is also something here");
    }
}
