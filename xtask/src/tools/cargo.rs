use anyhow::anyhow;
use camino::Utf8PathBuf;

use crate::commands::version::RoverVersion;
use crate::target::{TARGET_ARM_MACOS, TARGET_X86_64_MACOS, Target};
use crate::tools::{GitRunner, Runner};
use crate::utils::{self, CommandOutput, PKG_PROJECT_ROOT};
use crate::Result;

use std::collections::HashMap;
use std::fs;

pub(crate) struct CargoRunner {
    cargo_package_directory: Utf8PathBuf,
    runner: Runner,
}

impl CargoRunner {
    /// Creates a new cargo runner with knowledge of the root rover binary and all plugins
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let runner = Runner::new("cargo", verbose)?;
        Ok(CargoRunner {
            cargo_package_directory: PKG_PROJECT_ROOT.clone(),
            runner,
        })
    }

    /// builds all target binaries and returns their output directory
    pub(crate) fn build(
        &self,
        target: &Target,
        release: bool,
        version: Option<&RoverVersion>,
    ) -> Result<HashMap<String, Utf8PathBuf>> {
        if let Some(version) = version {
            let git_runner = GitRunner::new(self.runner.verbose)?;
            let repo_path = git_runner.checkout_rover_version(version.to_string().as_str())?;
            let versioned_schema_url = format!(
                    "https://github.com/apollographql/rover/releases/download/{0}/rover-{0}-schema.graphql",
                    &version);

            crate::info!("downloading schema from {}", &versioned_schema_url);
            let schema_response =
                reqwest::blocking::get(versioned_schema_url)?.error_for_status()?;
            let schema_text = schema_response.text()?;
            if !schema_text.contains("subgraph") {
                anyhow!("This schema doesn't seem to contain any references to `subgraph`s. It's probably the wrong schema.");
            }
            let schema_dir = repo_path
                .join("crates")
                .join("rover-client")
                .join(".schema");

            // do the first build in the cloned repo
            let _ = self.cargo_exec(
                vec![
                    "build",
                    "--manifest-path",
                    &repo_path.join("Cargo.toml").to_string(),
                ],
                vec![],
                Some(target),
                release,
            );

            // overwrite the schema with the one we downloaded, exit the loop and build again
            fs::write(schema_dir.join("schema.graphql"), schema_text)?;
        }

        self.cargo_exec(
            vec!["build", "--workspace", "--locked"],
            vec![],
            Some(target),
            release,
        )?;

        
        let bin_paths = utils::get_bin_paths(target, release);
        for (bin_name, bin_path) in &bin_paths {
            if matches!(target, Target::UniversalMacOS) {
                let lipo_runner = Runner::new("lipo", self.runner.verbose)?;
                let dbg_or_release = if release {
                    "release"
                } else {
                    "debug"
                };
                let target_dir = PKG_PROJECT_ROOT.join("target");
                let universal_output = target_dir.join(target.to_string()).join(dbg_or_release).join(bin_name);
                let x86_64_input = target_dir.join(TARGET_X86_64_MACOS).join(dbg_or_release).join(bin_name);
                let arm_input = target_dir.join(TARGET_ARM_MACOS).join(dbg_or_release).join(bin_name);
                fs::create_dir_all(&universal_output.ancestors().nth(1).expect("Could not find parent directory of ./target"))?;
                lipo_runner.exec(&["-create", "-output", universal_output.as_str(), x86_64_input.as_str(), arm_input.as_str()], &PKG_PROJECT_ROOT, None)?;
            }
            crate::info!("successfully compiled `{}` to `{}`", bin_name, bin_path);
        }
        Ok(bin_paths)
    }

    pub(crate) fn lint(&self) -> Result<()> {
        self.cargo_exec(vec!["fmt", "--all"], vec!["--check"], None, false)?;
        self.cargo_exec(vec!["clippy", "--all"], vec!["-D", "warnings"], None, false)?;

        Ok(())
    }

    pub(crate) fn update_deps(&self) -> Result<()> {
        self.cargo_exec(vec!["update"], vec![], None, false)?;
        self.cargo_exec(vec!["update"], vec![], None, false)?;
        Ok(())
    }

    pub(crate) fn test(&self, target: &Target) -> Result<()> {
        let release = false;
        let all_command_outputs = self.cargo_exec(
            vec!["test", "--workspace", "--locked"],
            vec![],
            Some(target),
            release,
        )?;
        // for some reason, cargo test doesn't actually fail if there are failed tests...????
        // so here we manually collect all the lines including failed tests and display them
        // as warnings for the dev.
        let mut failed_tests = Vec::new();

        for command_output in &all_command_outputs {
            for line in command_output.stdout.lines() {
                if line.starts_with("test") && line.contains("FAILED") {
                    failed_tests.push(line);
                }
            }
        }

        if !failed_tests.is_empty() {
            for failed_test in &failed_tests {
                let split_test: Vec<&str> = failed_test.splitn(3, ' ').collect();
                if split_test.len() < 3 {
                    panic!("Something went wrong with xtask's failed test detection.");
                }
                let exact_test = split_test[1];

                // drop the result here so we can re-run the failed tests and print their output.
                let _ = self.cargo_exec(
                    vec![
                        "test",
                        "--manifest-path",
                        &all_command_outputs[0].directory.join("Cargo.toml").to_string(),
                    ],
                    vec![exact_test, "--exact", "--nocapture"],
                    Some(target),
                    release,
                );
            }
            Err(anyhow!("`cargo test` failed {} times.", failed_tests.len()))
        } else {
            Ok(())
        }
    }

    fn cargo_exec(
        &self,
        cargo_args: Vec<&str>,
        extra_args: Vec<&str>,
        target: Option<&Target>,
        release: bool,
    ) -> Result<Vec<CommandOutput>> {
        let mut command_outputs = vec![];
        let mut command_args: Vec<String> = cargo_args.iter().map(|a| a.to_string()).collect();

        let target = target.unwrap_or_else(|| &Target::Other);

        let env = target.get_env()?;
        if release {
            command_args.push("--release".to_string());
        }

        if !extra_args.is_empty() {
            command_args.push("--".to_string());
            for extra_arg in extra_args {
                command_args.push(extra_arg.to_string());
            }
        }
        for cargo_args in target.get_all_cargo_args() {
            let mut these_args = command_args.clone();
            these_args.extend(cargo_args.clone());
            let these_args: Vec<&str> = these_args.iter().map(AsRef::as_ref).collect();
            command_outputs.push(self.runner.exec(&these_args, &self.cargo_package_directory, Some(&env))?);
        }

        Ok(command_outputs)
    }
}
