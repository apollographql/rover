use saucer::{anyhow, Context, Fs, Utf8PathBuf};

use std::collections::HashSet;
use std::net::ToSocketAddrs;
use std::sync::mpsc::Receiver;

use crate::command::dev::command::BackgroundTask;
use crate::command::dev::do_dev::log_err_and_continue;
use crate::command::dev::socket::{ComposeResult, SubgraphKey, SubgraphName, SubgraphUrl};
use crate::command::dev::SupergraphOpts;
use crate::command::install::Plugin;
use crate::command::Install;
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug)]
pub struct RouterRunner {
    supergraph_schema_path: Utf8PathBuf,
    router_config_path: Utf8PathBuf,
    plugin_opts: PluginOpts,
    supergraph_opts: SupergraphOpts,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    router_handle: Option<BackgroundTask>,
}

impl RouterRunner {
    pub fn new(
        supergraph_schema_path: Utf8PathBuf,
        router_config_path: Utf8PathBuf,
        plugin_opts: PluginOpts,
        supergraph_opts: SupergraphOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Self {
        Self {
            supergraph_schema_path,
            router_config_path,
            plugin_opts,
            supergraph_opts,
            override_install_path,
            client_config,
            router_handle: None,
        }
    }

    pub fn get_command_to_spawn(&self) -> Result<String> {
        let plugin = Plugin::Router;
        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepted: self.plugin_opts.elv2_license_accepted,
        };

        // maybe do the install, maybe find a pre-existing installation, maybe fail
        let exe = install_command
            .get_versioned_plugin(
                self.override_install_path.clone(),
                self.client_config.clone(),
                self.plugin_opts.skip_update,
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
        let contents = format!(
            r#"
        server:
          listen: {}
        plugins:
            experimental.include_subgraph_errors:
              all: true
            experimental.expose_query_plan: true
        "#,
            self.supergraph_opts.listen_addr()?
        );
        Ok(Fs::write_file(&self.router_config_path, contents, "")
            .context("could not create router config")?)
    }

    pub fn spawn(&mut self) -> Result<()> {
        if self.router_handle.is_none() {
            self.write_router_config()?;
            self.router_handle = Some(BackgroundTask::new(self.get_command_to_spawn()?)?);
            eprintln!(
                "router is running! head to http://{} to query your supergraph",
                &self.supergraph_opts.listen_addr()?
            );
        }
        Ok(())
    }

    pub fn kill(&mut self) -> Result<()> {
        if let Some(router_handle) = self.router_handle.as_mut() {
            router_handle.kill();
            self.router_handle = None;
        }
        Ok(())
    }

    pub fn kill_or_spawn(&mut self, compose_receiver: Receiver<ComposeResult>) -> ! {
        loop {
            let _ = match compose_receiver.recv().unwrap() {
                ComposeResult::Succeed => self.spawn(),
                ComposeResult::Fail | ComposeResult::Kill => self.kill(),
            }
            .map_err(log_err_and_continue);
        }
    }
}

impl Drop for RouterRunner {
    fn drop(&mut self) {
        if let Some(router_handle) = &self.router_handle {
            let message = format!("could not kill router with PID {}", router_handle.id());
            self.kill().expect(&message);
        }
    }
}
