use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

use anyhow::{anyhow, Context};
use crossbeam_channel::Sender;

use crate::{command::dev::do_dev::log_err_and_continue, RoverError, RoverResult};

#[derive(Debug)]
pub struct BackgroundTask {
    child: Child,
}

pub enum BackgroundTaskLog {
    Stdout(String),
    Stderr(String),
}

impl BackgroundTask {
    pub fn new(command: String, log_sender: Sender<BackgroundTaskLog>) -> RoverResult<Self> {
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

        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .with_context(|| "could not spawn child process")?;

        if let Some(stdout) = child.stdout.take() {
            let log_sender = log_sender.clone();
            rayon::spawn(move || {
                let stdout = BufReader::new(stdout);
                stdout.lines().for_each(|line| {
                    if let Ok(line) = line {
                        log_sender
                            .send(BackgroundTaskLog::Stdout(line))
                            .expect("could not update stdout logs for command");
                    }
                });
            });
        }

        if let Some(stderr) = child.stderr.take() {
            rayon::spawn(move || {
                let stderr = BufReader::new(stderr);
                stderr.lines().for_each(|line| {
                    if let Ok(line) = line {
                        log_sender
                            .send(BackgroundTaskLog::Stderr(line))
                            .expect("could not update stderr logs for command");
                    }
                });
            });
        }

        Ok(Self { child })
    }

    pub fn kill(&mut self) {
        let pid = self.id();
        tracing::info!("killing child with pid {}", &pid);
        let _ = self.child.kill().map_err(|_| {
            log_err_and_continue(RoverError::new(anyhow!(
                "could not kill child with pid {}",
                &pid
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
