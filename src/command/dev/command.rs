use std::process::{Child, Command, Stdio};

use saucer::{anyhow, Context};

use crate::Result;

#[derive(Debug)]
pub struct CommandRunner {
    tasks: Vec<BackgroundTask>,
}

impl CommandRunner {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn spawn(&mut self, command: String) -> Result<()> {
        let args: Vec<&str> = command.split(' ').collect();
        let (bin, args) = match args.len() {
            0 => Err(anyhow!("the command you passed is empty")),
            1 => Ok((args[0], Vec::new())),
            _ => Ok((args[0], Vec::from_iter(args[1..].iter()))),
        }?;
        eprintln!("starting `{}`", &command);
        if which::which(bin).is_ok() {
            let mut command = Command::new(bin);
            command.args(args);
            self.tasks.push(BackgroundTask::new(command)?);
            Ok(())
        } else {
            Err(anyhow!("{} is not installed on this machine", &bin).into())
        }
    }
}

impl Drop for CommandRunner {
    fn drop(&mut self) {
        eprintln!("dropping spawned background tasks");
        for background_task in self.tasks.iter_mut() {
            #[cfg(unix)]
            {
                // attempt to stop gracefully
                let pid = background_task.child.id();
                unsafe {
                    libc::kill(libc::pid_t::from_ne_bytes(pid.to_ne_bytes()), libc::SIGTERM);
                }

                for _ in 0..10 {
                    if background_task.child.try_wait().ok().flatten().is_some() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }

            if background_task.child.try_wait().ok().flatten().is_none() {
                // still alive? kill it with fire
                let _ = background_task.child.kill();
            }

            let _ = background_task.child.wait();
        }
    }
}

#[derive(Debug)]
struct BackgroundTask {
    child: Child,
}

impl BackgroundTask {
    fn new(mut command: Command) -> Result<Self> {
        if cfg!(windows) {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        eprintln!("spawning {:?}", &command);
        let child = command
            .spawn()
            .with_context(|| "Could not spawn child process")?;
        eprintln!("spawned...");
        Ok(Self { child })
    }
}
