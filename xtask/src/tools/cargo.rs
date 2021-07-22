use std::{collections::HashMap, str::FromStr};

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

use crate::target::Target;
use crate::tools::Runner;
use crate::utils::{self, CommandOutput};

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
        let target_str = target.to_string();
        let mut args = vec!["build", "--target", &target_str];
        if release {
            args.push("--release");
        }
        if !target.composition_js() {
            args.push("--no-default-features");
        }
        let mut env = HashMap::new();
        match target {
            Target::GnuLinux | Target::MuslLinux => {
                env.insert("OPENSSL_STATIC".to_string(), "1".to_string());
            }
            Target::MacOS => {
                let openssl_path = "/usr/local/opt/openssl@1.1".to_string();
                if Utf8PathBuf::from_str(&openssl_path)?.exists() {
                    env.insert("OPENSSL_DIR".to_string(), openssl_path);
                } else {
                    return Err(anyhow!("OpenSSL v1.1 is not installed. Please install with `brew install openssl@1.1`"));
                }
                env.insert("OPENSSL_STATIC".to_string(), "1".to_string());
            }
            Target::Windows => {
                env.insert(
                    "RUSTFLAGS".to_string(),
                    "-Ctarget-feature=+crt-static".to_string(),
                );
            }
        }
        self.cargo_exec(&args, Some(env))?;
        Ok(self.get_bin_path(target, release))
    }

    pub(crate) fn lint(&self) -> Result<()> {
        self.cargo_exec(&["fmt", "--all", "--", "--check"], None)?;
        self.cargo_exec(&["clippy", "--all", "--", "-D", "warnings"], None)?;
        self.cargo_exec(
            &[
                "clippy",
                "--all",
                "--no-default-features",
                "--",
                "-D",
                "warnings",
            ],
            None,
        )?;
        Ok(())
    }

    pub(crate) fn test(&self, target: Target) -> Result<()> {
        let target_str = target.to_string();
        let mut args = vec!["test", "--workspace", "--locked", "--target", &target_str];
        if !target.composition_js() {
            args.push("--no-default-features");
        }

        let command_output = self.cargo_exec(&args, None)?;

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
                let _ =
                    self.cargo_exec(&["test", "--", exact_test, "--exact", "--nocapture"], None);
            }
            Err(anyhow!("`cargo test` failed {} times.", failed_tests.len()))
        }
    }

    pub(crate) fn get_bin_path(&self, target: &Target, release: bool) -> Utf8PathBuf {
        let mut path = self.cargo_package_directory.clone();
        path.push("target");
        path.push(target.to_string());
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
        args: &[&str],
        env: Option<HashMap<String, String>>,
    ) -> Result<CommandOutput> {
        self.runner.exec(args, &self.cargo_package_directory, env)
    }
}
