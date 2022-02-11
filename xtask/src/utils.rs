use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use cargo_metadata::{Metadata, MetadataCommand};
use lazy_static::lazy_static;

use std::{collections::HashMap, convert::TryFrom, env, process::Output, str};

use crate::target::Target;

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
#[allow(dead_code)]
pub const PKG_PROJECT_NAME: &str = "rover";

lazy_static! {
    pub(crate) static ref PKG_VERSION: String =
        rover_version().expect("Could not find Rover's version.");
    pub(crate) static ref PKG_PROJECT_ROOT: Utf8PathBuf =
        project_root().expect("Could not find Rover's project root.");
    pub(crate) static ref TARGET_DIR: Utf8PathBuf = CARGO_METADATA.clone().target_directory;
    static ref CARGO_METADATA: Metadata = cargo_metadata().expect("Could not run `cargo metadata`");
}

#[macro_export]
macro_rules! info {
    ($msg:expr $(, $($tokens:tt)* )?) => {{
        let info_prefix = ansi_term::Colour::White.bold().paint("info:");
        eprintln!(concat!("{} ", $msg), &info_prefix $(, $($tokens)*)*);
    }};
}

fn rover_version() -> Result<String> {
    Ok(CARGO_METADATA
        .root_package()
        .ok_or_else(|| anyhow!("Could not find root package."))?
        .version
        .to_string())
}

fn project_root() -> Result<Utf8PathBuf> {
    let manifest_dir = Utf8PathBuf::try_from(MANIFEST_DIR)
        .with_context(|| "Could not find the root directory.")?;
    let root_dir = manifest_dir
        .ancestors()
        .nth(1)
        .ok_or_else(|| anyhow!("Could not find project root."))?;
    Ok(root_dir.to_path_buf())
}

fn cargo_metadata() -> Result<Metadata> {
    let metadata = MetadataCommand::new()
        .manifest_path(PKG_PROJECT_ROOT.join("Cargo.toml"))
        .no_deps()
        .exec()?;
    Ok(metadata)
}

pub(crate) fn get_bin_paths(crate_target: &Target, release: bool) -> HashMap<String, Utf8PathBuf> {
    let mut bin_paths = HashMap::new();
    for package in &CARGO_METADATA.packages {
        for target in &package.targets {
            for kind in &target.kind {
                if kind == "bin" && target.name != "xtask" {
                    let mut bin_path = CARGO_METADATA.target_directory.clone();
                    if !crate_target.is_other() {
                        bin_path.push(crate_target.to_string())
                    }
                    if release {
                        bin_path.push("release")
                    } else {
                        bin_path.push("debug")
                    };
                    bin_path.push(target.name.clone());
                    bin_paths.insert(target.name.clone(), bin_path);
                }
            }
        }
    }
    bin_paths
}

pub(crate) struct CommandOutput {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) _output: Output,
    pub(crate) directory: Utf8PathBuf,
}
