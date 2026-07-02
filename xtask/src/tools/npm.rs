use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;

use crate::{
    tools::Runner,
    utils::{CommandOutput, PKG_PROJECT_NAME, PKG_PROJECT_ROOT, PKG_VERSION},
};

pub(crate) struct NpmRunner {
    runner: Runner,
    npm_installer_package_directory: Utf8PathBuf,
}

impl NpmRunner {
    pub(crate) fn new() -> Result<Self> {
        let runner = Runner::new("npm");
        let project_root = PKG_PROJECT_ROOT.clone();

        let npm_installer_package_directory = project_root
            .join("installers")
            .join("npm")
            .join("@apollo")
            .join("rover");

        Ok(Self {
            runner,
            npm_installer_package_directory,
        })
    }

    /// prepares our npm installer package for release
    ///
    /// `stub` skips embedding cross-compiled binaries and omits
    /// `optionalDependencies`/`PLATFORMS`. Use `true` for local dry runs
    /// (e.g. `xtask prep`) where platform packages haven't been built or
    /// published yet; use `false` when actually publishing the package so
    /// it ships with a populated `PLATFORMS` map.
    pub(crate) fn prepare_package(&self, stub: bool) -> Result<()> {
        self.generate_packages(stub)
            .with_context(|| "Could not generate npm packages.")?;

        self.patch_shim()
            .with_context(|| "Could not patch npm shim.")?;

        self.install_dependencies()
            .with_context(|| "Could not install dependencies.")?;

        self.publish_dry_run()
            .with_context(|| "Publish dry-run failed.")?;

        Ok(())
    }

    fn generate_packages(&self, stub: bool) -> Result<()> {
        let runner = Runner::new("cargo");
        // -p is required: without it, `cargo npm generate` fails with
        // "no targets configured" instead of reading [package.metadata.npm]
        // from the workspace root's own package.
        let mut args: Vec<&str> = vec!["npm", "generate", "-p", PKG_PROJECT_NAME];
        if stub {
            // --stub generates the main @apollo/rover wrapper package without cross-compiled
            // binaries or optionalDependencies. Platform packages (@apollo/rover-{os}-{cpu})
            // are generated per-target in CI.
            args.push("--stub");
        }
        runner.exec(&args, &PKG_PROJECT_ROOT, None)?;
        Ok(())
    }

    fn patch_shim(&self) -> Result<()> {
        let shim_path = self
            .npm_installer_package_directory
            .join("bin")
            .join("rover.js");
        let content = std::fs::read_to_string(&shim_path)
            .with_context(|| format!("Could not read shim at {}", shim_path))?;
        let patched = content.replace(
            "const bin = require.resolve(binPath)",
            "const bin = require.resolve(binPath)\nprocess.env.APOLLO_NODE_MODULES_BIN_DIR = require('path').dirname(bin)",
        );
        if patched == content {
            anyhow::bail!(
                "patch-npm-shim: marker not found — shim may have already been patched or changed format"
            );
        }
        std::fs::write(&shim_path, patched)
            .with_context(|| format!("Could not write shim at {}", shim_path))?;
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

    pub(crate) fn publish(
        &self,
        package_dir: &Utf8PathBuf,
        dry_run: bool,
        npm_tag: Option<&str>,
    ) -> Result<()> {
        // npm stage publish — staged so the release can be inspected before going live.
        let mut args: Vec<&str> = vec!["stage", "publish", "--access", "public"];
        if let Some(tag) = npm_tag {
            args.extend(["--tag", tag]);
        }
        if dry_run {
            args.push("--dry-run");
        }
        self.npm_exec(&args, package_dir)?;
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
