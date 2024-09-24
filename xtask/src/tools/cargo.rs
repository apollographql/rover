use std::fs;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

use crate::commands::version::RoverVersion;
use crate::target::Target;
use crate::tools::{GitRunner, Runner};
use crate::utils::{CommandOutput, PKG_PROJECT_ROOT};

pub(crate) struct CargoRunner {
    cargo_package_directory: Utf8PathBuf,
    runner: Runner,
}

impl CargoRunner {
    /// Creates a new cargo runner with knowledge of the root rover binary and all plugins
    pub(crate) fn new() -> Result<Self> {
        let runner = Runner::new("cargo");
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
    ) -> Result<Utf8PathBuf> {
        if let Some(version) = version {
            let git_runner = GitRunner::tmp()?;
            let repo_path = git_runner.checkout_rover_version(version.to_string().as_str())?;
            let versioned_schema_url = format!(
                    "https://github.com/apollographql/rover/releases/download/{0}/rover-{0}-schema.graphql",
                    &version);
            crate::info!("downloading schema from {}", &versioned_schema_url);
            let schema_response =
                reqwest::blocking::get(versioned_schema_url)?.error_for_status()?;
            let schema_text = schema_response.text()?;
            if !schema_text.contains("subgraph") {
                return Err(anyhow!("This schema doesn't seem to contain any references to `subgraph`s. It's probably the wrong schema."));
            }
            let schema_dir = repo_path
                .join("crates")
                .join("rover-client")
                .join(".schema");

            // do the first build in the cloned repo
            let mut cargo_args = vec![
                "build".to_string(),
                "--manifest-path".to_string(),
                repo_path.join("Cargo.toml").to_string(),
            ];
            if release {
                cargo_args.push("--release".to_string())
            }
            let _ = self.cargo_exec(
                cargo_args.iter().map(|s| s.as_ref()).collect(),
                vec!["--nocapture"],
                Some(target),
            );

            // overwrite the schema with the one we downloaded, exit the loop and build again
            fs::write(schema_dir.join("schema.graphql"), schema_text)?;
        }
        let mut cargo_args = vec!["build", "--workspace"];
        if release {
            cargo_args.push("--release");
            cargo_args.push("--locked");
        }
        self.cargo_exec(cargo_args, vec![], Some(target))?;
        let bin_path = target.get_bin_path(release);
        crate::info!("successfully compiled to `{}`", &bin_path);
        Ok(bin_path)
    }

    pub(crate) fn lint(&self) -> Result<()> {
        self.cargo_exec(vec!["fmt", "--all"], vec!["--check"], None)?;
        self.cargo_exec(vec!["clippy", "--all"], vec!["-D", "warnings"], None)?;

        Ok(())
    }

    pub(crate) fn security_check(&self) -> Result<()> {
        self.cargo_exec(vec!["deny", "check", "licenses"], vec![], None)?;
        self.cargo_exec(vec!["deny", "check", "advisories"], vec![], None)?;

        Ok(())
    }

    pub(crate) fn update_deps(&self) -> Result<()> {
        self.cargo_exec(vec!["update"], vec![], None)?;
        self.cargo_exec(vec!["update"], vec![], None)?;
        Ok(())
    }

    pub(crate) fn test(&self, target: &Target) -> Result<()> {
        let command_output = self.cargo_exec(
            vec!["test", "--workspace", "--locked"],
            vec![],
            Some(target),
        )?;

        // for some reason, cargo test doesn't actually fail if there are failed tests...????
        // so here we manually collect all the lines including failed tests and display them
        // as warnings for the dev.
        let mut failed_tests = Vec::new();

        for line in command_output.stdout.lines() {
            if line.starts_with("test") && line.contains("FAILED") {
                failed_tests.push(line);
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
                        command_output.directory.join("Cargo.toml").as_ref(),
                    ],
                    vec![exact_test, "--exact", "--nocapture"],
                    Some(target),
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
    ) -> Result<CommandOutput> {
        let mut cargo_args = cargo_args
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>();
        let mut env = None;
        if let Some(target) = target {
            cargo_args.extend(target.get_cargo_args());
            env = Some(target.get_env()?);
        };
        if !extra_args.is_empty() {
            cargo_args.push("--".to_string());
            for extra_arg in extra_args {
                cargo_args.push(extra_arg.to_string());
            }
        }
        self.runner.exec(
            &cargo_args.iter().map(AsRef::as_ref).collect::<Vec<&str>>(),
            &self.cargo_package_directory,
            env.as_ref(),
        )
    }
}
