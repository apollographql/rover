use crate::utils::client::StudioClientConfig;
use crate::{anyhow, command::RoverOutput, error::RoverError, Context, Result};
use crate::{Suggestion, PKG_VERSION};

use apollo_federation_types::{BuildError, BuildErrors};
use apollo_supergraph_config::SupergraphConfig;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use tempdir::TempDir;

use std::convert::TryFrom;
use std::env::consts::EXE_SUFFIX;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::str;

const FEDERATION_PLUGIN: &str = "rover-fed2";

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
}

impl Compose {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let binary_name = format!("{}{}", FEDERATION_PLUGIN, EXE_SUFFIX);
        let exe = which::which(&binary_name).map_err(|_| {
            let mut err = RoverError::new(anyhow!(
                "You must have {}@v{} installed and in your PATH to run this command.",
                FEDERATION_PLUGIN,
                PKG_VERSION
            ));
            if cfg!(target_os = "windows") {
                err.set_suggestion(Suggestion::Adhoc(format!("You can install {} by running `iwr https://rover.apollo.dev/plugins/{}/win/v{} | iex` in a PowerShell terminal.", FEDERATION_PLUGIN, FEDERATION_PLUGIN, PKG_VERSION)));
            } else if !cfg!(target_os = "windows") && !cfg!(target_arch = "musl") {
                err.set_suggestion(Suggestion::Adhoc(format!("You can install {} by running `curl https://rover.apollo.dev/plugins/{}/nix/v{} | sh`", FEDERATION_PLUGIN, FEDERATION_PLUGIN, PKG_VERSION)));
            } else if cfg!(target_os = "linux") && cfg!(target_arch = "musl") {
                err.set_suggestion(Suggestion::Adhoc("Unfortunately, this plugin is not supported on musl architectures. You'll need to switch to a glibc Linux distribution in order to run this command.".to_string()));
            }
            err
        })?;

        let version_output = Command::new(&exe)
            .arg("--version")
            .output()
            .context("Could not run `rover-fed2 --version`")?;
        let version_stdout = str::from_utf8(&version_output.stdout)
            .context("`rover-fed2 --version` output is not valid UTF-8")?;
        let first_line = version_stdout
            .lines()
            .next()
            .ok_or_else(|| anyhow!("`rover-fed2 --version` output malformed."))?;
        let split_version: Vec<&str> = first_line.split(' ').collect();
        if split_version.len() >= 2 {
            if split_version[1] != PKG_VERSION {
                Err(anyhow!("The version of `rover-fed2` you have installed is {} while the version of rover you have installed is {}. These must be the same.", split_version[1], PKG_VERSION))
            } else {
                Ok(())
            }
        } else {
            Err(anyhow!(
                "Could not find version from `rover-fed2 --version`"
            ))
        }?;

        let subgraph_definitions = crate::command::supergraph::get_subgraph_definitions(
            &self.config_path,
            client_config,
            &self.profile_name,
        )?;
        let supergraph_config: SupergraphConfig = subgraph_definitions.into();
        let supergraph_config_yaml = serde_yaml::to_string(&supergraph_config)?;
        let dir = TempDir::new(FEDERATION_PLUGIN)?;
        tracing::debug!("temp dir created at {}", dir.path().display());
        let yaml_path = Utf8PathBuf::try_from(dir.path().join("config.yml"))?;
        let mut f = File::create(&yaml_path)?;
        f.write_all(supergraph_config_yaml.as_bytes())?;
        f.sync_all()?;
        tracing::debug!("config file written to {}", &yaml_path);
        let output = Command::new(&exe)
            .args(&["compose", &yaml_path.to_string(), "--json"])
            .output()
            .context("Failed to execute command")?;
        let stdout = str::from_utf8(&output.stdout).with_context(|| {
            format!("Could not parse output of `{} compose`", FEDERATION_PLUGIN)
        })?;
        if let Ok(composition_output) = serde_json::from_str::<CompositionOutput>(stdout) {
            return Ok(RoverOutput::CompositionResult {
                hints: composition_output.hints,
                supergraph_sdl: composition_output.supergraph_sdl,
            });
        } else if let Ok(error_message) = serde_json::from_str::<GenericError>(stdout) {
            return Err(RoverError::new(anyhow!("{}", error_message.message)));
        } else if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(stdout) {
            if let Some(array) = json_value.as_array() {
                let mut build_errors = BuildErrors::new();
                for item in array {
                    if let Ok(build_error) = serde_json::from_str::<BuildError>(&item.to_string()) {
                        build_errors.push(build_error);
                    } else {
                        break;
                    }
                }
                if !build_errors.is_empty() {
                    return Err(RoverError::new(build_errors));
                }
            }
        };

        Err(RoverError::new(anyhow!(
            "Output from `{} compose` was malformed.",
            FEDERATION_PLUGIN,
        )))
    }
}

#[derive(Deserialize, Serialize)]
struct GenericError {
    message: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompositionOutput {
    hints: Vec<String>,
    supergraph_sdl: String,
}
