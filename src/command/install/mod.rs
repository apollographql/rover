use serde::Serialize;
use structopt::StructOpt;

use binstall::Installer;

use crate::command::RoverStdout;
use crate::PKG_NAME;
use crate::{anyhow, Context, Result};

use camino::Utf8PathBuf;
use std::env;

#[derive(Debug, Serialize, StructOpt)]
pub struct Install {
    #[structopt(long = "force", short = "f")]
    force: bool,
}

impl Install {
    pub fn run(&self, override_install_path: Option<Utf8PathBuf>) -> Result<RoverStdout> {
        let binary_name = PKG_NAME.to_string();
        if let Ok(executable_location) = env::current_exe() {
            let executable_location = Utf8PathBuf::from_path_buf(executable_location)
                .map_err(|pb| anyhow!("File path \"{}\" is not valid UTF-8", pb.display()))?;
            let installer = Installer {
                binary_name: binary_name.clone(),
                force_install: self.force,
                override_install_path,
                executable_location,
            };
            let install_location = installer
                .install()
                .with_context(|| format!("could not install {}", &binary_name))?;

            if let Some(_) = install_location {
                let bin_dir_path = installer.get_bin_dir_path()?;
                eprintln!("{} was successfully installed. Great!", &binary_name);
                if !cfg!(windows) {
                    if let Some(path_var) = env::var_os("PATH") {
                        if !path_var
                            .to_string_lossy()
                            .to_string()
                            .contains(bin_dir_path.as_str())
                        {
                            eprintln!("\nTo get started you need Rover's bin directory ({}) in your PATH environment variable. Next time you log in this will be done automatically.", &bin_dir_path);
                            if let Ok(shell_var) = env::var("SHELL") {
                                eprintln!(
                                    "\nTo configure your current shell, you can run:\nexec {}",
                                    &shell_var
                                );
                            }
                        }
                    }
                }
            } else {
                eprintln!("{} was not installed. To override the existing installation, you can pass the `--force` flag to the installer.", &binary_name);
            }
            Ok(RoverStdout::None)
        } else {
            Err(anyhow!("Failed to get the current executable's path.").into())
        }
    }
}
