use std::str;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;

use crate::{
    tools::Runner,
    utils::{CommandOutput, PKG_PROJECT_ROOT, PKG_VERSION},
};

const PLATFORM_PACKAGE_DIRS: &[&str] = &[
    "rover-darwin-arm64",
    "rover-darwin-x64",
    "rover-linux-arm64",
    "rover-linux-x64",
    "rover-linux-x64-musl",
    "rover-win32-x64",
];

pub(crate) struct NpmRunner {
    runner: Runner,
    npm_installer_package_directory: Utf8PathBuf,
    platforms_directory: Utf8PathBuf,
}

impl NpmRunner {
    pub(crate) fn new() -> Result<Self> {
        let runner = Runner::new("npm");
        let project_root = PKG_PROJECT_ROOT.clone();

        let rover_client_lint_directory = project_root.join("crates").join("rover-client");
        let npm_installer_package_directory = project_root.join("installers").join("npm");
        let platforms_directory = npm_installer_package_directory.join("platforms");

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

        Ok(Self {
            runner,
            npm_installer_package_directory,
            platforms_directory,
        })
    }

    /// prepares our npm installer package for release
    pub(crate) fn prepare_package(&self) -> Result<()> {
        self.update_dependency_tree()
            .with_context(|| "Could not update the dependency tree.")?;

        self.update_version()
            .with_context(|| "Could not update Rover's version in package.json.")?;

        self.update_platform_package_versions()
            .with_context(|| "Could not update platform package versions.")?;

        self.install_dependencies()
            .with_context(|| "Could not install dependencies.")?;

        self.publish_dry_run()
            .with_context(|| "Publish dry-run failed.")?;

        Ok(())
    }

    fn update_dependency_tree(&self) -> Result<()> {
        self.npm_exec(&["update"], &self.npm_installer_package_directory)?;
        Ok(())
    }

    fn install_dependencies(&self) -> Result<()> {
        // --ignore-scripts so we do not attempt to run any postinstall hooks
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

    /// Bumps the version in every platform package and syncs the
    /// optionalDependencies version refs in the main package.json.
    fn update_platform_package_versions(&self) -> Result<()> {
        for dir_name in PLATFORM_PACKAGE_DIRS {
            let pkg_dir = self.platforms_directory.join(dir_name);
            self.npm_exec(&["version", &PKG_VERSION, "--allow-same-version"], &pkg_dir)
                .with_context(|| {
                    format!("Could not update version in platform package: {}", dir_name)
                })?;
        }

        self.sync_optional_dep_versions()
            .with_context(|| "Could not sync optionalDependencies versions in package.json.")
    }

    /// Reads the main package.json, updates every value under optionalDependencies
    /// to PKG_VERSION, and writes it back.
    fn sync_optional_dep_versions(&self) -> Result<()> {
        let pkg_json_path = self
            .npm_installer_package_directory
            .join("package.json")
            .into_std_path_buf();

        let contents = std::fs::read_to_string(&pkg_json_path)
            .with_context(|| format!("Could not read {}", pkg_json_path.display()))?;

        let mut json: serde_json::Value = serde_json::from_str(&contents)
            .with_context(|| "Could not parse package.json as JSON")?;

        if let Some(optional_deps) = json
            .get_mut("optionalDependencies")
            .and_then(|v| v.as_object_mut())
        {
            for value in optional_deps.values_mut() {
                *value = serde_json::Value::String(PKG_VERSION.clone());
            }
        }

        let updated = serde_json::to_string_pretty(&json)
            .with_context(|| "Could not serialize package.json")?;

        std::fs::write(&pkg_json_path, updated + "\n")
            .with_context(|| format!("Could not write {}", pkg_json_path.display()))?;

        Ok(())
    }

    fn publish_dry_run(&self) -> Result<()> {
        let version = semver::Version::parse(&PKG_VERSION).with_context(|| {
            format!(
                "Could not parse Rover version '{}' as semver.",
                *PKG_VERSION
            )
        })?;
        let mut args: Vec<&str> = vec!["publish", "--dry-run"];
        if !version.pre.is_empty() {
            args.extend(["--tag", "beta"]);
        }
        let command_output = self.npm_exec(&args, &self.npm_installer_package_directory)?;

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
