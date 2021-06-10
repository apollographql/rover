use ansi_term::Colour::White;
use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use cargo_metadata::MetadataCommand;
use lazy_static::lazy_static;
use which::which;

use std::{
    collections::HashMap,
    convert::TryFrom,
    env,
    process::{Command, Output},
    str,
};

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
        .nth(1)
        .ok_or_else(|| anyhow!("Could not find project root."))?;
    Ok(root_dir.to_path_buf())
}

pub(crate) fn exec(
    command_name: &str,
    args: &[&str],
    directory: &Utf8PathBuf,
    verbose: bool,
    env: Option<HashMap<String, String>>,
) -> Result<CommandOutput> {
    let command_path = which(command_name).with_context(|| {
        format!(
            "You must have {} installed to run this command.",
            &command_name
        )
    })?;
    let full_command = format!("`{} {}`", command_name, args.join(" "));
    info(&format!("running {}", &full_command));
    let mut command = Command::new(command_path);
    command.current_dir(directory).args(args);

    if let Some(env) = env {
        for (key, value) in env {
            command.env(&key, &value);
        }
    }

    let output = command.output()?;
    let command_was_successful = output.status.success();
    let stdout = str::from_utf8(&output.stdout)
        .context("Command's stdout was not valid UTF-8.")?
        .to_string();
    let stderr = str::from_utf8(&output.stderr)
        .context("Command's stderr was not valid UTF-8.")?
        .to_string();
    if verbose || !command_was_successful {
        if !stderr.is_empty() {
            eprintln!("{}", &stderr);
        }
        if !stdout.is_empty() {
            println!("{}", &stdout);
        }
    }

    if command_was_successful {
        Ok(CommandOutput {
            _stdout: stdout,
            stderr,
            _output: output,
        })
    } else if let Some(exit_code) = output.status.code() {
        Err(anyhow!(
            "{} exited with status code {}",
            &full_command,
            exit_code
        ))
    } else {
        Err(anyhow!("{} was terminated by a signal.", &full_command))
    }
}

pub(crate) struct CommandOutput {
    pub(crate) _stdout: String,
    pub(crate) stderr: String,
    pub(crate) _output: Output,
}
