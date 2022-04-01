use crate::command::supergraph::resolve_supergraph_yaml;
use crate::utils::client::StudioClientConfig;
use crate::{
    anyhow,
    command::{
        install::{license_accept, Install, Plugin},
        RoverOutput,
    },
    error::{RoverError, Suggestion},
    Context, Result, PKG_NAME,
};

use apollo_federation_types::build::BuildResult;
use rover_client::RoverClientError;

use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;
use tempdir::TempDir;

use std::{
    fs::File,
    io::Write,
    process::Command,
    str::{self, FromStr},
};

const FEDERATION_PLUGIN: &str = "supergraph";

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
    #[structopt(long = "elv2-license", parse(from_str = license_accept), case_insensitive = true)]
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
        let supergraph_config =
            resolve_supergraph_yaml(&self.config_path, client_config.clone(), &self.profile_name)?;
        let supergraph_with_major = format!(
            "{}-{}",
            FEDERATION_PLUGIN,
            supergraph_config.get_federation_version()
        );
        let plugin = Plugin::from_str(&supergraph_with_major)?;
        let versioned_plugin = if !self.skip_update {
            let install_command = Install {
                force: false,
                plugin: Some(plugin),
                elv2_license_accepted: self.elv2_license_accepted,
            };

            let installer = install_command
                .get_installer(PKG_NAME.to_string(), override_install_path.clone())?;
            let latest_version = installer.get_plugin_version(&plugin.get_tarball_url()?)?;
            let plugin_name = plugin.get_name();
            let versioned_plugin = format!("{}-{}", &plugin_name, &latest_version);
            let maybe_exe = which::which(&versioned_plugin);
            if maybe_exe.is_err() {
                tracing::debug!("{} does not exist, installing it", &plugin_name);
                eprintln!(
                    "updating 'rover supergraph compose' to use {}...",
                    &versioned_plugin
                );
                install_command.run(override_install_path, client_config)?;
            } else {
                tracing::debug!("{} exists, skipping install", &versioned_plugin);
            }
            versioned_plugin
        } else {
            // TODO: look in ~/.rover/bin for all `supergraph-v` and find one w/the highest appropriate major version
            // and error if one does not exist.
            "supergraph-v2.0.0-preview.9".to_string()
        };

        let exe = which::which(&versioned_plugin)?;

        let supergraph_config_yaml = serde_yaml::to_string(&supergraph_config)?;
        let dir = TempDir::new(FEDERATION_PLUGIN)?;
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
        let stdout = str::from_utf8(&output.stdout).with_context(|| {
            format!("Could not parse output of `{} compose`", &versioned_plugin)
        })?;

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
                .with_context(|| anyhow!("{} compose output: {}", &versioned_plugin, stdout))
                .with_context(|| {
                    anyhow!("Output from `{} compose` was malformed.", &versioned_plugin)
                })
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
