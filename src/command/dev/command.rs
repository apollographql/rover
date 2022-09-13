use std::process::{Command, Stdio};

use command_group::{CommandGroup, GroupChild};
use saucer::{anyhow, Context};

use crate::{command::dev::do_dev::log_err_and_continue, error::RoverError, Result};

#[derive(Debug)]
pub struct BackgroundTask {
    child: GroupChild,
}

impl BackgroundTask {
    pub fn new(command: String) -> Result<Self> {
        let args: Vec<&str> = command.split(' ').collect();
        let (bin, args) = match args.len() {
            0 => Err(anyhow!("the command you passed is empty")),
            1 => Ok((args[0], Vec::new())),
            _ => Ok((args[0], Vec::from_iter(args[1..].iter()))),
        }?;
        tracing::info!("starting `{}`", &command);
        if which::which(bin).is_err() {
            return Err(anyhow!("{} is not installed on this machine", &bin).into());
        }

        let mut command = Command::new(bin);
        command.args(args).env("APOLLO_ROVER", "true");

        if cfg!(windows) {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        let child = command
            .group_spawn()
            .with_context(|| "could not spawn child process")?;
        Ok(Self { child })
    }

    pub fn kill(&mut self) {
        let pgid = self.id();
        tracing::info!("killing child with pgid {}", &pgid);
        let _ = self.child.kill().map_err(|_| {
            log_err_and_continue(RoverError::new(anyhow!(
                "could not kill child group with pgid {}",
                &pgid
            )));
        });
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for BackgroundTask {
    fn drop(&mut self) {
        self.kill()
    }
}
