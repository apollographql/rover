use crate::utils::CommandOutput;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use which::which;

use std::collections::HashMap;
use std::convert::TryInto;
use std::process::{Command, Output, Stdio};
use std::str;

pub(crate) struct Runner {
    pub(crate) verbose: bool,
    pub(crate) tool_name: String,
    pub(crate) tool_exe: Utf8PathBuf,
}

impl Runner {
    pub(crate) fn new(tool_name: &str, verbose: bool) -> Result<Self> {
        let tool_exe = which(tool_name).with_context(|| {
            format!(
                "You must have {} installed to run this command.",
                &tool_name
            )
        })?;
        Ok(Runner {
            verbose,
            tool_name: tool_name.to_string(),
            tool_exe: tool_exe.try_into()?,
        })
    }

    pub(crate) fn exec(
        &self,
        args: &[&str],
        directory: &Utf8PathBuf,
        env: Option<&HashMap<String, String>>,
    ) -> Result<CommandOutput> {
        let full_command = format!("`{} {}`", &self.tool_name, args.join(" "));
        crate::info!("running {} in `{}`", &full_command, directory);
        if self.verbose {
            if let Some(env) = env {
                crate::info!("env:");
                for (key, value) in env {
                    crate::info!("  ${}={}", key, value);
                }
            }
        }

        let mut command = Command::new(&self.tool_exe);
        command
            .current_dir(directory)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(env) = env {
            command.envs(env);
        }
        let child = command
            .spawn()
            .with_context(|| "Could not spawn child process")?;
        let output = child
            .wait_with_output()
            .with_context(|| "Failed to wait for child process to exit")?;
        self.handle_command_output(output, directory)
            .with_context(|| format!("Encountered an issue while executing {}", &full_command))
    }

    fn handle_command_output(
        &self,
        output: Output,
        directory: &Utf8PathBuf,
    ) -> Result<CommandOutput> {
        let command_was_successful = output.status.success();
        let stdout = str::from_utf8(&output.stdout)
            .context("Command's stdout was not valid UTF-8.")?
            .to_string();
        let stderr = str::from_utf8(&output.stderr)
            .context("Command's stderr was not valid UTF-8.")?
            .to_string();
        if self.verbose || !command_was_successful {
            if !stderr.is_empty() {
                eprintln!("{}", &stderr);
            }
            if !stdout.is_empty() {
                println!("{}", &stdout);
            }
        }

        if command_was_successful {
            Ok(CommandOutput {
                stdout,
                stderr,
                _output: output,
                directory: directory.clone(),
            })
        } else if let Some(exit_code) = output.status.code() {
            Err(anyhow!("Exited with status code {}", exit_code))
        } else {
            Err(anyhow!("Terminated by a signal."))
        }
    }
}
