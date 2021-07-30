use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

use crate::target::Target;
use crate::tools::Runner;
use crate::utils::{self, CommandOutput};

pub(crate) struct CargoRunner {
    cargo_package_directory: Utf8PathBuf,
    runner: Runner,
    target: Target,
}

impl CargoRunner {
    pub(crate) fn new(target: Target, verbose: bool) -> Result<Self> {
        let runner = Runner::new("cargo", verbose)?;
        let cargo_package_directory = utils::project_root()?;

        Ok(CargoRunner {
            cargo_package_directory,
            runner,
            target,
        })
    }

    pub(crate) fn build(&self, release: bool) -> Result<Utf8PathBuf> {
        let args = vec!["build"];
        self.cargo_exec(args, vec![], release)?;
        let bin_path = self.get_bin_path(release);
        utils::info(&format!("successfully compiled to `{}`", &bin_path));
        Ok(bin_path)
    }

    pub(crate) fn lint(&self) -> Result<()> {
        self.cargo_exec(vec!["fmt", "--all"], vec!["--check"], false)?;
        self.cargo_exec(vec!["clippy", "--all"], vec!["-D", "warnings"], false)?;
        Ok(())
    }

    pub(crate) fn test(&self) -> Result<()> {
        let command_output =
            self.cargo_exec(vec!["test", "--workspace", "--locked"], vec![], false)?;

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
                let _ = self.cargo_exec(
                    vec!["test"],
                    vec![exact_test, "--exact", "--nocapture"],
                    false,
                );
            }
            Err(anyhow!("`cargo test` failed {} times.", failed_tests.len()))
        }
    }

    pub(crate) fn get_bin_path(&self, release: bool) -> Utf8PathBuf {
        let mut path = self.cargo_package_directory.clone();
        if !self.target.is_other() {
            path.push("target");
            path.push(self.target.to_string());
        }
        if release {
            path.push("release")
        } else {
            path.push("debug")
        }
        path.push("rover");
        path
    }

    fn cargo_exec(
        &self,
        cargo_args: Vec<&str>,
        extra_args: Vec<&str>,
        release: bool,
    ) -> Result<CommandOutput> {
        let target_args = self.target.get_args();
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
        let env = self.target.get_env()?;

        if !extra_args.is_empty() {
            cargo_args.push("--");
            cargo_args.extend(extra_args);
        }

        self.runner
            .exec(&cargo_args, &self.cargo_package_directory, env.as_ref())
    }
}
