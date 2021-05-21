use ansi_term::Colour::White;
use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;

use std::{convert::TryFrom, env, process::Output, str};

pub(crate) use rover::PKG_VERSION;
const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub(crate) fn info(msg: &str) {
    let info_prefix = White.bold().paint("info:");
    eprintln!("{} {}", &info_prefix, msg);
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

pub(crate) fn process_command_output(output: &Output) -> Result<()> {
    if !output.status.success() {
        let stdout =
            str::from_utf8(&output.stdout).context("Command's stdout was not valid UTF-8.")?;
        let stderr =
            str::from_utf8(&output.stderr).context("Command's stderr was not valid UTF-8.")?;
        eprintln!("{}", stderr);
        println!("{}", stdout);
        Err(anyhow!("Could not run command."))
    } else {
        Ok(())
    }
}
