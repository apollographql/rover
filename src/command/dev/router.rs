use saucer::{anyhow, Context, Fs, Utf8PathBuf};

use std::collections::HashSet;
use std::net::ToSocketAddrs;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::{thread, time::Duration};

use crate::command::dev::command::CommandRunner;
use crate::command::dev::command::CommandRunnerMessage;
use crate::command::dev::do_dev::log_err_and_continue;
use crate::command::dev::socket::{ComposeResult, SubgraphKey, SubgraphName, SubgraphUrl};
use crate::command::install::Plugin;
use crate::command::Install;
use crate::error::RoverError;
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Clone)]
pub struct RouterRunner {
    supergraph_schema_path: Utf8PathBuf,
    router_config_path: Utf8PathBuf,
    opts: PluginOpts,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    is_spawned: bool,
}

impl RouterRunner {
    pub fn new(
        supergraph_schema_path: Utf8PathBuf,
        router_config_path: Utf8PathBuf,
        opts: PluginOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Self {
        Self {
            supergraph_schema_path,
            router_config_path,
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
            "{} --supergraph {} --hot-reload --config {} --log {}",
            &exe,
            self.supergraph_schema_path.as_str(),
            self.router_config_path.as_str(),
            self.log_level()
        ))
    }

    fn log_level(&self) -> &str {
        "info"
    }

    fn write_router_config(&self) -> Result<()> {
        let contents = r#"
        plugins:
            experimental.include_subgraph_errors:
              all: true
        "#;
        Ok(Fs::write_file(&self.router_config_path, contents, "")
            .context("could not create router config")?)
    }

    pub fn spawn(&mut self, command_sender: Sender<CommandRunnerMessage>) -> Result<()> {
        if !self.is_spawned {
            self.write_router_config()?;
            let (ready_sender, ready_receiver) = CommandRunner::ready_channel();
            command_sender.send(CommandRunnerMessage::SpawnTask {
                subgraph_name: Self::reserved_subgraph_name(),
                command: self.get_command_to_spawn()?,
                ready_sender,
            })?;
            ready_receiver.recv()?;
            let client = self.client_config.get_reqwest_client()?;
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
                }
                thread::sleep(Duration::from_millis(400));
            }
            eprintln!("router is running! head to http://localhost:4000 to query your supergraph");
            Ok(())
        } else {
            Err(RoverError::new(anyhow!(
                "router is already spawned, not respawning"
            )))
        }
    }

    pub fn kill(&mut self, command_sender: Sender<CommandRunnerMessage>) -> Result<()> {
        if !self.is_spawned {
            Err(RoverError::new(anyhow!(
                "router is not spawned, so there is nothing to kill"
            )))
        } else {
            let (kill_sender, kill_receiver) = CommandRunner::ready_channel();
            command_sender.send(CommandRunnerMessage::KillTask {
                subgraph_name: Self::reserved_subgraph_name(),
                ready_sender: kill_sender,
            })?;
            kill_receiver.recv()?;
            self.is_spawned = false;
            Ok(())
        }
    }

    pub fn endpoints() -> HashSet<SubgraphUrl> {
        "localhost:4000"
            .to_socket_addrs()
            .map(|sas| {
                sas.filter_map(|s| {
                    format!("http://{}:{}", s.ip(), s.port())
                        .parse::<SubgraphUrl>()
                        .ok()
                })
                .collect()
            })
            .unwrap_or_else(|_| HashSet::new())
    }

    pub fn reserved_subgraph_name() -> SubgraphName {
        "__apollo__router__rover__dev__if__you__use__this__subgraph__name__something__might__go__wrong".to_string()
    }

    pub fn reserved_subgraph_keys() -> HashSet<SubgraphKey> {
        let name = Self::reserved_subgraph_name();
        Self::endpoints()
            .iter()
            .cloned()
            .map(|endpoint| (name.to_string(), endpoint))
            .collect()
    }

    pub fn kill_or_spawn(
        &mut self,
        command_sender: Sender<CommandRunnerMessage>,
        compose_receiver: Receiver<ComposeResult>,
    ) -> ! {
        loop {
            let _ = match compose_receiver.recv().unwrap() {
                ComposeResult::Succeed => self.spawn(command_sender.clone()),
                ComposeResult::Fail => self.kill(command_sender.clone()),
            }
            .map_err(log_err_and_continue);
        }
    }
}
