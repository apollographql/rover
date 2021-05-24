use ansi_term::Colour::White;
use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use cargo_metadata::MetadataCommand;
use lazy_static::lazy_static;

use std::{convert::TryFrom, env, str};

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

lazy_static! {
    pub(crate) static ref PKG_VERSION: String =
        rover_version().expect("Could not find Rover's version.");
}

pub(crate) fn info(msg: &str) {
    let info_prefix = White.bold().paint("info:");
    eprintln!("{} {}", &info_prefix, msg);
}

pub(crate) fn rover_version() -> Result<String> {
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

pub(crate) fn project_root() -> Result<Utf8PathBuf> {
    let manifest_dir = Utf8PathBuf::try_from(MANIFEST_DIR)
        .with_context(|| "Could not find the root directory.")?;
    let root_dir = manifest_dir
        .ancestors()
        .nth(2)
        .ok_or_else(|| anyhow!("Could not find project root."))?;
    Ok(root_dir.to_path_buf())
}
