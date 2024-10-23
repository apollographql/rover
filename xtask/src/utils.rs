use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use cargo_metadata::{Metadata, MetadataCommand};
use lazy_static::lazy_static;

use std::{env, str};

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
#[allow(dead_code)]
pub const PKG_PROJECT_NAME: &str = "rover";

lazy_static! {
    pub(crate) static ref PKG_VERSION: String =
        rover_version().expect("Could not find Rover's version.");
    pub(crate) static ref PKG_PROJECT_ROOT: Utf8PathBuf =
        project_root().expect("Could not find Rover's project root.");
    pub(crate) static ref TARGET_DIR: Utf8PathBuf = CARGO_METADATA.clone().target_directory;
    static ref CARGO_METADATA_WITHOUT_DEPS: Metadata =
        cargo_metadata_without_deps().expect("Could not run `cargo metadata`");
    static ref CARGO_METADATA: Metadata = cargo_metadata().expect("Could not run `cargo metadata`");
}

#[macro_export]
macro_rules! info {
    ($msg:expr $(, $($tokens:tt)* )?) => {{
        let info_prefix = console::style("info:").white().bold();
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
    println!("in project root");

    let mut manifest_dir = MANIFEST_DIR.to_string();

    match std::env::var("NIX_CARGO_MANIFEST_DIR") {
        Ok(v) => {
            println!("nix cargo manifest dir: {v}");
            manifest_dir = v;
        }
        Err(e) => {
            println!("error finding nix env var: {e}");
        }
    }

    let manifest_dir = Utf8PathBuf::from(manifest_dir);
    println!("manifest_dir: {manifest_dir}");

    let root_dir = manifest_dir
        .ancestors()
        .nth(1)
        .ok_or_else(|| anyhow!("Could not find project root."))?;

    println!("root dir: {root_dir:?}");
    Ok(root_dir.to_path_buf())
}

fn cargo_metadata() -> Result<Metadata> {
    let metadata = MetadataCommand::new()
        .manifest_path(PKG_PROJECT_ROOT.join("Cargo.toml"))
        .exec()?;
    Ok(metadata)
}

fn cargo_metadata_without_deps() -> Result<Metadata> {
    let metadata = MetadataCommand::new()
        .manifest_path(PKG_PROJECT_ROOT.join("Cargo.toml"))
        .no_deps()
        .exec()?;
    Ok(metadata)
}

pub(crate) struct CommandOutput {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) directory: Utf8PathBuf,
}
