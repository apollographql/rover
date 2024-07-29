use std::ffi::OsString;
use std::fs::OpenOptions;
use std::{fs, str};

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use regex::Regex;
use which::which;

use rover_std::Fs;

use crate::info;
use crate::{
    tools::LatestPluginVersions,
    tools::Runner,
    utils::{CommandOutput, PKG_PROJECT_ROOT, PKG_VERSION},
};

pub(crate) struct NpmRunner {
    runner: Runner,
    npm_installer_package_directory: Utf8PathBuf,
    rover_client_lint_directory: Utf8PathBuf,
    flyby_directory: Utf8PathBuf,
}

impl NpmRunner {
    pub(crate) fn new() -> Result<Self> {
        let runner = Runner::new("npm");
        let project_root = PKG_PROJECT_ROOT.clone();

        let rover_client_lint_directory = project_root.join("crates").join("rover-client");
        let npm_installer_package_directory = project_root.join("installers").join("npm");
        let flyby_directory = project_root.join("examples").join("flyby");

        if !npm_installer_package_directory.exists() {
            return Err(anyhow!(
                "Rover's npm installer package does not seem to be located here:\n{}",
                &npm_installer_package_directory
            ));
        }

        if !rover_client_lint_directory.exists() {
            return Err(anyhow!(
                "Rover's GraphQL linter package does not seem to be located here:\n{}",
                &rover_client_lint_directory
            ));
        }

        if !flyby_directory.exists() {
            return Err(anyhow!(
                "Rover's example flyby directory does not seem to be located here:\n{}",
                &flyby_directory
            ));
        }

        Ok(Self {
            runner,
            npm_installer_package_directory,
            rover_client_lint_directory,
            flyby_directory,
        })
    }

    /// prepares our npm installer package for release
    /// you must have volta installed to run this command
    pub(crate) fn prepare_package(&self) -> Result<()> {
        self.require_volta()?;

        self.update_dependency_tree()
            .with_context(|| "Could not update the dependency tree.")?;

        self.update_version()
            .with_context(|| "Could not update Rover's version in package.json.")?;

        self.install_dependencies()
            .with_context(|| "Could not install dependencies.")?;

        self.publish_dry_run()
            .with_context(|| "Publish dry-run failed.")?;

        Ok(())
    }

    pub(crate) fn dev_docs(&self, dir: &Utf8PathBuf) -> Result<()> {
        self.require_volta()?;
        if fs::metadata(dir.join("node_modules")).is_err() {
            self.npm_exec(&["i"], dir)?;
        }
        crate::info!("serving './docs' at http://localhost:8000/rover");
        self.npm_exec(&["run", "start:local"], dir)?;
        Ok(())
    }

    pub(crate) fn update_linter(&self) -> Result<()> {
        self.npm_exec(&["update"], &self.rover_client_lint_directory)?;
        Ok(())
    }

    pub(crate) fn lint(&self) -> Result<()> {
        self.require_volta()?;
        self.npm_exec(&["install"], &self.rover_client_lint_directory)?;
        self.npm_exec(&["run", "lint"], &self.rover_client_lint_directory)?;

        Ok(())
    }

    // this command runs integration tests with a test account in Apollo Studio with the flyby demo
    pub(crate) fn flyby(&self) -> Result<()> {
        let run_studio_tests = || -> Result<()> {
            self.require_volta()?;
            self.npm_exec(&["install"], &self.flyby_directory)?;
            self.npm_exec(&["run", "compose:file"], &self.flyby_directory)?;
            self.npm_exec(&["run", "compose:graphref"], &self.flyby_directory)?;
            self.npm_exec(&["run", "compose:introspect"], &self.flyby_directory)?;
            self.npm_exec(&["run", "compose:broken"], &self.flyby_directory)?;
            self.npm_exec(&["run", "locations:check"], &self.flyby_directory)?;
            self.npm_exec(&["run", "locations:publish"], &self.flyby_directory)?;
            self.npm_exec(&["run", "locations:fetch"], &self.flyby_directory)?;
            self.npm_exec(&["run", "reviews:check"], &self.flyby_directory)?;
            self.npm_exec(&["run", "reviews:publish"], &self.flyby_directory)?;
            self.npm_exec(&["run", "reviews:fetch"], &self.flyby_directory)?;
            self.npm_exec(&["run", "broken:check"], &self.flyby_directory)?;
            Ok(())
        };
        if std::env::var_os("FLYBY_APOLLO_KEY").is_some()
            || Fs::assert_path_exists(PKG_PROJECT_ROOT.join("examples").join("flyby").join(".env"))
                .is_ok()
        {
            if let Some(val) = std::env::var_os("LATEST_FED_VERSION_JSON_KEY") {
                let json_key = match val.to_str() {
                    Some("latest-0") => "latest-0",
                    Some("latest-2") => "latest-2",
                    Some(_) | None => {
                        info!("Environment variable LATEST_FED_VERSION_TO_USE_FROM_FILE should only contain 'latest-0' or 'latest-2', could not read or misconfigured defaulting to 'latest-2'");
                        "latest-2"
                    }
                };
                self.set_supergraph_yaml_files_correctly(json_key)?;
            }
            run_studio_tests()
        } else if std::env::var_os("CIRCLE_PR_NUMBER").is_some() {
            // this environment variable is only set by CircleCI for forked PRs
            // https://circleci.com/docs/variables#built-in-environment-variables
            info!("skipping studio integration tests because this is a forked repository without a $FLYBY_APOLLO_KEY");
            Ok(())
        } else {
            Err(anyhow!(
                "$FLYBY_APOLLO_KEY is not set and this does not appear to be a forked PR. This API key should have permissions to run checks on the `flyby-rover` graph (https://studio.apollographql.com/graph/flyby-rover) and it can be set in ./examples/flyby/.env."
            ))
        }
    }

    fn set_supergraph_yaml_files_correctly(&self, json_key: &str) -> Result<()> {
        let fed_version = self.get_federation_version(json_key)?;
        info!("Running tests with Federation version {}", fed_version);
        self.update_supergraph_yaml_files(fed_version)?;
        Ok(())
    }

    fn update_supergraph_yaml_files(&self, fed_version: String) -> Result<()> {
        let mut supergraphs_path = self.flyby_directory.clone();
        supergraphs_path.push("supergraphs");
        for dir_entry in fs::read_dir(supergraphs_path)? {
            let path = dir_entry.unwrap().path();
            if [OsString::from("yaml"), OsString::from("yml")]
                .contains(&path.extension().unwrap().to_ascii_lowercase())
            {
                // Open the file once to pull out the data
                let file = OpenOptions::new().read(true).open(&path)?;
                let mut value: serde_yaml::Value = serde_yaml::from_reader(&file)?;
                value["federation_version"] = format!("={}", fed_version).into();
                // Open the file again to ensure the write is clean
                let file = OpenOptions::new().write(true).truncate(true).open(path)?;
                serde_yaml::to_writer(file, &value)?;
            }
        }
        Ok(())
    }

    fn get_federation_version(&self, json_key: &str) -> Result<String> {
        let mut latest_plugin_versions_path = self.flyby_directory.clone();
        latest_plugin_versions_path.push("../../latest_plugin_versions.json");
        let data = fs::read_to_string(latest_plugin_versions_path)?;
        let latest_plugin_versions: LatestPluginVersions = serde_json::from_str(&data)?;
        let re = Regex::new(r"v(.*)").unwrap();
        let final_version = re
            .captures(
                latest_plugin_versions
                    .supergraph
                    .versions
                    .get(json_key)
                    .unwrap(),
            )
            .unwrap()
            .get(1)
            .unwrap();
        Ok(String::from(final_version.as_str()))
    }

    fn require_volta(&self) -> Result<()> {
        which("volta")
            .map(|_| ())
            .map_err(|_| anyhow!("You must have `volta` installed to run this command."))
    }

    fn update_dependency_tree(&self) -> Result<()> {
        self.npm_exec(&["update"], &self.npm_installer_package_directory)?;
        Ok(())
    }

    fn install_dependencies(&self) -> Result<()> {
        // we --ignore-scripts so that we do not attempt to download and unpack a
        // released rover tarball
        self.npm_exec(
            &["install", "--ignore-scripts"],
            &self.npm_installer_package_directory,
        )?;
        Ok(())
    }

    fn update_version(&self) -> Result<()> {
        self.npm_exec(
            &["version", &PKG_VERSION, "--allow-same-version"],
            &self.npm_installer_package_directory,
        )?;
        Ok(())
    }

    fn publish_dry_run(&self) -> Result<()> {
        let command_output = self.npm_exec(
            &["publish", "--dry-run"],
            &self.npm_installer_package_directory,
        )?;

        assert_publish_includes(&command_output)
            .with_context(|| "There were problems with the output of 'npm publish --dry-run'.")
    }

    fn npm_exec(&self, args: &[&str], directory: &Utf8PathBuf) -> Result<CommandOutput> {
        self.runner.exec(args, directory, None)
    }
}

fn assert_publish_includes(output: &CommandOutput) -> Result<()> {
    let necessary_files = vec!["LICENSE", "README.md"];
    let mut missing_files = Vec::with_capacity(necessary_files.len());

    for necessary_file in necessary_files {
        if !output.stderr.contains(necessary_file) {
            missing_files.push(necessary_file);
        }
    }

    if missing_files.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "The npm tarball is missing the following files: {:?}",
            &missing_files
        ))
    }
}
