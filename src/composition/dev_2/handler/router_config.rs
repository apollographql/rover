use camino::Utf8PathBuf;
use rover_std::{Fs, RoverStdError};

#[derive(thiserror::Error, Debug)]
pub enum RouterConfigError {
    #[error("Unable to write router config file")]
    FailedToWriteFile(RoverStdError),
}

pub struct WriteRouterConfig {
    path: Utf8PathBuf,
}

impl WriteRouterConfig {
    pub fn new(path: Utf8PathBuf) -> WriteRouterConfig {
        WriteRouterConfig { path }
    }
    pub fn run(&self, contents: &str) -> Result<(), RouterConfigError> {
        Fs::write_file(self.path.clone(), contents.to_string())
            .map_err(|err| RouterConfigError::FailedToWriteFile(err))
    }
}
