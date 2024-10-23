use camino::Utf8PathBuf;
use shell_candy::{ShellTask, ShellTaskBehavior, ShellTaskLog, ShellTaskOutput};

use std::collections::HashMap;

use crate::{utils::CommandOutput, Result};

pub struct Runner {
    pub(crate) bin: String,
    pub(crate) override_bash_descriptor: Option<String>,
}

impl Runner {
    pub fn new(bin: &str) -> Self {
        Self {
            bin: bin.to_string(),
            override_bash_descriptor: None,
        }
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn set_bash_descriptor(&mut self, new_bash_descriptor: String) {
        self.override_bash_descriptor = Some(new_bash_descriptor);
    }

    fn get_bash_descriptor(&self, task: &ShellTask) -> String {
        self.override_bash_descriptor
            .clone()
            .unwrap_or_else(|| task.bash_descriptor())
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
        println!("directory: {directory:?}");
        task.current_dir(directory);
        if let Some(env) = env {
            for (k, v) in env {
                task.env(k, v);
            }
        }
        let bin = self.bin.to_string();
        println!("bin as string: {bin}");
        crate::info!("{}", &self.get_bash_descriptor(&task));
        let task_result = task.run(move |line| {
            match line {
                ShellTaskLog::Stdout(line) | ShellTaskLog::Stderr(line) => {
                    crate::info!("({bin}) | {line}", bin = bin, line = line);
                }
            }
            ShellTaskBehavior::<()>::Passthrough
        })?;

        println!("F2");
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
