use crate::utils::{self, CommandOutput};

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use which::which;

use std::collections::HashMap;
use std::convert::TryInto;
use std::process::{Command, Output};
use std::str;

pub(crate) struct Runner {
    verbose: bool,
    tool_name: String,
    tool_exe: Utf8PathBuf,
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
        directory: &Utf8Path,
        env: Option<HashMap<String, String>>,
    ) -> Result<CommandOutput> {
        let full_command = format!("`{} {}`", &self.tool_name, args.join(" "));
        utils::info(&format!("running {}", &full_command));

        let mut command = Command::new(&self.tool_exe);
        command.current_dir(directory).args(args);
        self.set_command_env(&mut command, env);
        let output = command.output()?;
        self.handle_command_output(output)
            .with_context(|| format!("Encountered an issue while executing {}", &full_command))
    }

    fn set_command_env(&self, command: &mut Command, env: Option<HashMap<String, String>>) {
        if let Some(env) = env {
            for (key, value) in env {
                command.env(&key, &value);
            }
        }
    }

    fn handle_command_output(&self, output: Output) -> Result<CommandOutput> {
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
                _stdout: stdout,
                stderr,
                _output: output,
            })
        } else if let Some(exit_code) = output.status.code() {
            Err(anyhow!("Exited with status code {}", exit_code))
        } else {
            Err(anyhow!("Terminated by a signal."))
        }
    }
}
