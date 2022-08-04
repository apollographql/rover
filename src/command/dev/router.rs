use saucer::{anyhow, Utf8PathBuf};

use std::{thread, time::Duration};

use crate::command::dev::command::CommandRunner;
use crate::command::install::Plugin;
use crate::command::Install;
use crate::error::RoverError;
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Clone)]
pub struct RouterRunner {
    read_path: Utf8PathBuf,
    opts: PluginOpts,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    is_spawned: bool,
}

impl RouterRunner {
    pub fn new(
        read_path: Utf8PathBuf,
        opts: PluginOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Self {
        Self {
            read_path,
            opts,
            override_install_path,
            client_config,
            is_spawned: false,
        }
    }

    pub fn get_command_to_spawn(&self) -> Result<String> {
        let plugin = Plugin::Router;
        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepted: self.opts.elv2_license_accepted,
        };

        // maybe do the install, maybe find a pre-existing installation, maybe fail
        let exe = install_command
            .get_versioned_plugin(
                self.override_install_path.clone(),
                self.client_config.clone(),
                self.opts.skip_update,
            )
            .map_err(|e| anyhow!("{}", e))?;

        Ok(format!(
            "{} --supergraph {} --hot-reload",
            &exe,
            self.read_path.as_str()
        ))
    }

    pub fn spawn(&mut self, command_runner: &mut CommandRunner) -> Result<()> {
        if !self.is_spawned {
            command_runner.spawn("__apollo__router__rover__dev__if__you__use__this__subgraph__name__something__might__go__wrong".to_string(), self.get_command_to_spawn()?)?;
            let client = self.client_config.get_reqwest_client();
            while !self.is_spawned {
                if let Ok(request) = client
                    .get("http://localhost:4000/.well-known/apollo/server-health")
                    .build()
                {
                    if let Ok(response) = client.execute(request) {
                        if response.error_for_status().is_ok() {
                            self.is_spawned = true;
                        }
                    }
                } else {
                    thread::sleep(Duration::from_millis(400));
                }
            }
            eprintln!("router is running! head to http://localhost:4000 to query your supergraph");
            Ok(())
        } else {
            Err(RoverError::new(anyhow!(
                "router is already spawned, not respawning"
            )))
        }
    }
}
