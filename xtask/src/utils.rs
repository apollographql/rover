use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use cargo_metadata::MetadataCommand;
use lazy_static::lazy_static;

use std::{convert::TryFrom, env, process::Output, str};

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
#[allow(dead_code)]
pub const PKG_PROJECT_NAME: &str = "rover";

lazy_static! {
    pub(crate) static ref PKG_VERSION: String =
        rover_version().expect("Could not find Rover's version.");
    pub(crate) static ref PKG_PROJECT_ROOT: Utf8PathBuf =
        project_root().expect("Could not find Rover's project root.");
    pub(crate) static ref TARGET_DIR: Utf8PathBuf =
        target_dir().expect("Could not find Rover's target dir.");
}

#[macro_export]
macro_rules! info {
    ($msg:expr $(, $($tokens:tt)* )?) => {{
        let info_prefix = ansi_term::Colour::White.bold().paint("info:");
        eprintln!(concat!("{} ", $msg), &info_prefix $(, $($tokens)*)*);
    }};
}

fn rover_version() -> Result<String> {
    let project_root = project_root()?;
    let metadata = MetadataCommand::new()
        .manifest_path(project_root.join("Cargo.toml"))
        .exec()?;

    Ok(metadata
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

fn target_dir() -> Result<Utf8PathBuf> {
    let metadata = MetadataCommand::new()
        .manifest_path(PKG_PROJECT_ROOT.join("Cargo.toml"))
        .exec()?;

    Ok(metadata.target_directory)
}

pub(crate) struct CommandOutput {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) directory: Utf8PathBuf,
    pub(crate) _output: Output,
}
