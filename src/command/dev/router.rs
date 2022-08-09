use saucer::{anyhow, Context, Fs, Utf8PathBuf};

use std::collections::HashSet;
use std::net::ToSocketAddrs;
use std::{thread, time::Duration};

use crate::command::dev::command::CommandRunner;
use crate::command::dev::socket::{SubgraphKey, SubgraphName, SubgraphUrl};
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
            "{} --supergraph {} --hot-reload --config {}",
            &exe,
            self.supergraph_schema_path.as_str(),
            self.router_config_path.as_str()
        ))
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

    pub fn spawn(&mut self, command_runner: &mut CommandRunner) -> Result<()> {
        if !self.is_spawned {
            self.write_router_config()?;
            command_runner.spawn(
                &self.reserved_subgraph_name(),
                &self.get_command_to_spawn()?,
            )?;
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

    pub fn endpoints(&self) -> HashSet<SubgraphUrl> {
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

    pub fn reserved_subgraph_name(&self) -> SubgraphName {
        "__apollo__router__rover__dev__if__you__use__this__subgraph__name__something__might__go__wrong".to_string()
    }

    pub fn reserved_subgraph_keys(&self) -> HashSet<SubgraphKey> {
        self.endpoints()
            .iter()
            .cloned()
            .map(|e| (self.reserved_subgraph_name(), e))
            .collect()
    }
}
