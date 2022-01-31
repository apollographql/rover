use anyhow::anyhow;
use camino::Utf8PathBuf;

use crate::commands::version::RoverVersion;
use crate::target::Target;
use crate::tools::{GitRunner, Runner};
use crate::utils::{CommandOutput, PKG_PROJECT_NAME, PKG_PROJECT_ROOT, TARGET_DIR};
use crate::Result;

use std::fs;

pub(crate) struct CargoRunner {
    cargo_package_directory: Utf8PathBuf,
    plugin_directories: Vec<Utf8PathBuf>,
    runner: Runner,
}

impl CargoRunner {
    /// Creates a new cargo runner with knowledge of the root rover binary and all plugins
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let runner = Runner::new("cargo", verbose)?;
        let mut plugin_directories = Vec::new();
        for entry in fs::read_dir(PKG_PROJECT_ROOT.join("plugins"))? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::metadata(&path)?;
            if metadata.is_dir() {
                plugin_directories.push(Utf8PathBuf::try_from(path)?)
            }
        }
        Ok(CargoRunner {
            cargo_package_directory: PKG_PROJECT_ROOT.clone(),
            plugin_directories,
            runner,
        })
    }

    /// builds all target binaries including plugins
    pub(crate) fn build(
        &self,
        target: &Target,
        release: bool,
        version: Option<&RoverVersion>,
    ) -> Result<Vec<Utf8PathBuf>> {
        let mut binary_paths =
            vec![self.build_binary(target, release, version, &self.cargo_package_directory)?];
        for plugin_dir in &self.plugin_directories {
            binary_paths.push(self.build_binary(target, release, version, plugin_dir)?)
        }
        Ok(binary_paths)
    }

    fn build_binary(
        &self,
        target: &Target,
        release: bool,
        version: Option<&RoverVersion>,
        cargo_dir: &Utf8PathBuf,
    ) -> Result<Utf8PathBuf> {
        let binary_name = cargo_dir
            .file_stem()
            .ok_or_else(|| anyhow!("Could not find binary name of `{}`", cargo_dir))?
            .to_string();
        if binary_name == PKG_PROJECT_NAME {
            // if --version was passed, clone the repo and download the tagged schema
            // from Rover's GitHub releases
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
                fs::write(schema_dir.join("schema.graphql"), schema_text)?;
            }
        }

        self.cargo_exec(
            vec![
                "build",
                "--bin",
                &binary_name,
                "--manifest-path",
                &cargo_dir.join("Cargo.toml").to_string(),
            ],
            vec![],
            Some(target),
            release,
        )?;
        let bin_path = self.get_bin_path(target, release, &binary_name)?;
        crate::info!("successfully compiled to `{}`", &bin_path);
        Ok(bin_path)
    }

    pub(crate) fn lint(&self) -> Result<()> {
        self.cargo_exec_all(vec!["fmt", "--all"], vec!["--check"], None, false)?;
        self.cargo_exec_all(vec!["clippy", "--all"], vec!["-D", "warnings"], None, false)?;

        Ok(())
    }

    pub(crate) fn update_deps(&self) -> Result<()> {
        self.cargo_exec_all(vec!["update"], vec![], None, false)?;
        self.cargo_exec_all(vec!["update"], vec![], None, false)?;
        Ok(())
    }

    pub(crate) fn test(&self, target: &Target) -> Result<()> {
        let release = false;
        let command_outputs = self.cargo_exec_all(
            vec!["test", "--workspace", "--locked"],
            vec![],
            Some(target),
            release,
        )?;

        for command_output in command_outputs {
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
                            &command_output.directory.join("Cargo.toml").to_string(),
                        ],
                        vec![exact_test, "--exact", "--nocapture"],
                        Some(target),
                        release,
                    );
                }
                return Err(anyhow!("`cargo test` failed {} times.", failed_tests.len()));
            }
        }
        Ok(())
    }

    // this function outputs the path of a compiled binary given the values of cargo `--target` and `--release` options
    pub(crate) fn get_bin_path(
        &self,
        target: &Target,
        release: bool,
        bin_name: &str,
    ) -> Result<Utf8PathBuf> {
        let mut bin_path = Utf8PathBuf::try_from(TARGET_DIR.to_string())?;

        // if this is a known target, we pass `--target` to the cargo command
        // this means the output path will include the target string (like x86_64-unknown-linux-musl)
        if !target.is_other() {
            bin_path.push(target.to_string());
        }

        if release {
            // if `--release` is passed, the binary will be in the `release` subdirectory
            bin_path.push("release");
        } else {
            // otherwise it will be in the `debug` subdirectory
            bin_path.push("debug");
        }

        // finally, the binary name is of course the last part of the path
        bin_path.push(bin_name);

        Ok(bin_path)
    }

    fn cargo_exec(
        &self,
        cargo_args: Vec<&str>,
        extra_args: Vec<&str>,
        target: Option<&Target>,
        release: bool,
    ) -> Result<CommandOutput> {
        let mut command_args: Vec<String> = cargo_args.iter().map(|a| a.to_string()).collect();

        let env = if let Some(target) = target {
            // add explicit `--target` option
            command_args.extend(target.get_args());

            // use the root `target` even for plugins
            command_args.push("--target-dir".to_string());
            command_args.push(TARGET_DIR.to_string());

            // set target-specific environment variables
            Some(target.get_env()?)
        } else {
            None
        };

        if release {
            command_args.push("--release".to_string());
        }

        if !extra_args.is_empty() {
            command_args.push("--".to_string());
            for extra_arg in extra_args {
                command_args.push(extra_arg.to_string());
            }
        }

        let command_args: Vec<&str> = command_args.iter().map(AsRef::as_ref).collect();

        self.runner
            .exec(&command_args, &self.cargo_package_directory, env.as_ref())
    }

    // this function executes cargo commands in all package projects including plugins
    fn cargo_exec_all(
        &self,
        cargo_args: Vec<&str>,
        extra_args: Vec<&str>,
        target: Option<&Target>,
        release: bool,
    ) -> Result<Vec<CommandOutput>> {
        let mut command_outputs =
            vec![self.cargo_exec(cargo_args.clone(), extra_args.clone(), target, release)?];
        for plugin_dir in &self.plugin_directories {
            let manifest_path = plugin_dir.join("Cargo.toml").to_string();
            let path_args = vec!["--manifest-path", &manifest_path];
            command_outputs.push(self.cargo_exec(
                [&cargo_args[..], &path_args[..]].concat(),
                extra_args.clone(),
                target,
                release,
            )?);
        }
        Ok(command_outputs)
    }
}
