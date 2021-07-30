use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

use crate::target::Target;
use crate::tools::Runner;
use crate::utils::{self, CommandOutput};

use std::collections::HashMap;

pub(crate) struct CargoRunner {
    cargo_package_directory: Utf8PathBuf,
    runner: Runner,
}

impl CargoRunner {
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let runner = Runner::new("cargo", verbose)?;
        let cargo_package_directory = utils::project_root()?;

        Ok(CargoRunner {
            cargo_package_directory,
            runner,
        })
    }

    pub(crate) fn build(&self, target: &Target, release: bool) -> Result<Utf8PathBuf> {
        let args = vec!["build"];
        self.cargo_exec_with_target(target, args, vec![], release)?;
        let bin_path = self.get_bin_path(target, release);
        utils::info(&format!("successfully compiled to `{}`", &bin_path));
        Ok(bin_path)
    }

    pub(crate) fn lint(&self) -> Result<()> {
        self.cargo_exec_without_target(vec!["fmt", "--all"], vec!["--check"])?;
        self.cargo_exec_without_target(vec!["clippy", "--all"], vec!["-D", "warnings"])?;
        Ok(())
    }

    pub(crate) fn test(&self, target: &Target) -> Result<()> {
        let command_output = self.cargo_exec_with_target(
            target,
            vec!["test", "--workspace", "--locked"],
            vec![],
            false,
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

        if failed_tests.is_empty() {
            Ok(())
        } else {
            for failed_test in &failed_tests {
                let split_test: Vec<&str> = failed_test.splitn(3, ' ').collect();
                if split_test.len() < 3 {
                    panic!("Something went wrong with xtask's failed test detection.");
                }
                let exact_test = split_test[1];

                // drop the result here so we can re-run the failed tests and print their output.
                let _ = self.cargo_exec_with_target(
                    target,
                    vec!["test"],
                    vec![exact_test, "--exact", "--nocapture"],
                    false,
                );
            }
            Err(anyhow!("`cargo test` failed {} times.", failed_tests.len()))
        }
    }

    pub(crate) fn get_bin_path(&self, target: &Target, release: bool) -> Utf8PathBuf {
        let mut path = self.cargo_package_directory.clone();
        if target.is_other() {
            path.push("target");
            path.push(target.to_string());
        }
        if release {
            path.push("release")
        } else {
            path.push("debug")
        }
        path.push("rover");
        path
    }

    fn _cargo_exec(
        &self,
        cargo_args: Vec<&str>,
        extra_args: Vec<&str>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<CommandOutput> {
        let mut args = cargo_args;
        if !extra_args.is_empty() {
            args.push("--");
            for extra_arg in extra_args {
                args.push(extra_arg);
            }
        }

        self.runner.exec(&args, &self.cargo_package_directory, env)
    }

    fn cargo_exec_without_target(
        &self,
        cargo_args: Vec<&str>,
        extra_args: Vec<&str>,
    ) -> Result<CommandOutput> {
        self._cargo_exec(cargo_args, extra_args, None)
    }

    fn cargo_exec_with_target(
        &self,
        target: &Target,
        cargo_args: Vec<&str>,
        extra_args: Vec<&str>,
        release: bool,
    ) -> Result<CommandOutput> {
        let target_args = target.get_args();
        let mut cargo_args = cargo_args;
        cargo_args.extend(
            target_args
                .iter()
                .map(|target_arg| target_arg.as_str())
                .collect::<Vec<_>>(),
        );
        if release {
            cargo_args.push("--release");
        }
        let env = target.get_env()?;
        self._cargo_exec(cargo_args, extra_args, env.as_ref())
    }
}
