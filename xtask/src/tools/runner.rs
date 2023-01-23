use camino::Utf8PathBuf;
use shell_candy::{ShellTask, ShellTaskBehavior, ShellTaskLog, ShellTaskOutput};

use std::collections::HashMap;

use crate::{utils::CommandOutput, Result};

pub struct Runner {
    pub(crate) bin: String,
}

impl Runner {
    pub fn new(bin: &str) -> Self {
        Self {
            bin: bin.to_string(),
        }
    }

    pub(crate) fn exec(
        &self,
        args: &[&str],
        directory: &Utf8PathBuf,
        env: Option<&HashMap<String, String>>,
    ) -> Result<CommandOutput> {
        let mut task = ShellTask::new(&format!(
            "{bin} {args}",
            bin = &self.bin,
            args = args.join(" ")
        ))?;
        task.current_dir(directory);
        if let Some(env) = env {
            for (k, v) in env {
                task.env(k, v);
            }
        }
        let bin = self.bin.to_string();
        crate::info!("{}", task.bash_descriptor());
        let task_result = task.run(move |line| {
            match line {
                ShellTaskLog::Stdout(line) | ShellTaskLog::Stderr(line) => {
                    crate::info!("({bin}) | {line}", bin = bin, line = line);
                }
            }
            ShellTaskBehavior::<()>::Passthrough
        })?;
        match task_result {
            ShellTaskOutput::CompleteOutput {
                status: _,
                stdout_lines,
                stderr_lines,
            }
            | ShellTaskOutput::EarlyReturn {
                stdout_lines,
                stderr_lines,
                return_value: _,
            } => Ok(CommandOutput {
                stdout: stdout_lines.join("\n"),
                stderr: stderr_lines.join("\n"),
                directory: directory.clone(),
            }),
        }
    }
}
