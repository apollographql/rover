use std::{collections::HashMap, str::FromStr};

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

use crate::commands::Target;
use crate::utils::{self, CommandOutput};

pub(crate) struct CargoRunner {
    rover_package_directory: Utf8PathBuf,
    verbose: bool,
}

impl CargoRunner {
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let rover_package_directory = utils::project_root()?;

        Ok(CargoRunner {
            rover_package_directory,
            verbose,
        })
    }

    pub(crate) fn build(&self, target: Target) -> Result<Utf8PathBuf> {
        let target_str = target.to_string();
        let mut args = vec!["build", "--release", "--target", &target_str];
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
        Ok(self
            .rover_package_directory
            .join("target")
            .join(&target_str)
            .join("release")
            .join("rover"))
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
        let mut env = HashMap::new();
        env.insert("RUST_BACKTRACE".to_string(), "1".to_string());
        self.cargo_exec(&args, Some(env))?;

        Ok(())
    }

    fn cargo_exec(
        &self,
        args: &[&str],
        env: Option<HashMap<String, String>>,
    ) -> Result<CommandOutput> {
        utils::exec(
            "cargo",
            args,
            &self.rover_package_directory,
            self.verbose,
            env,
        )
    }
}
