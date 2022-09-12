use apollo_federation_types::config::RouterVersion;
use reqwest::blocking::Client;
use saucer::{anyhow, Context, Fs, Utf8PathBuf};
use semver::Version;

use std::time::{Duration, Instant};

use crate::command::dev::command::BackgroundTask;
use crate::command::dev::do_dev::log_err_and_continue;
use crate::command::dev::{SupergraphOpts, DEV_ROUTER_VERSION};
use crate::command::install::Plugin;
use crate::command::Install;
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::{error::RoverError, Result};

#[derive(Debug)]
pub struct RouterRunner {
    supergraph_schema_path: Utf8PathBuf,
    router_config_path: Utf8PathBuf,
    plugin_opts: PluginOpts,
    supergraph_opts: SupergraphOpts,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    router_handle: Option<BackgroundTask>,
    plugin_exe: Option<Utf8PathBuf>,
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
            plugin_exe: None,
        }
    }

    fn install_command(&self) -> Result<Install> {
        let plugin = Plugin::Router(RouterVersion::Exact(Version::parse(DEV_ROUTER_VERSION)?));
        Ok(Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepter: self.plugin_opts.elv2_license_accepter,
        })
    }

    pub fn maybe_install_router(&mut self) -> Result<Utf8PathBuf> {
        if let Some(plugin_exe) = &self.plugin_exe {
            Ok(plugin_exe.clone())
        } else {
            let install_command = self.install_command()?;
            let plugin_exe = install_command.get_versioned_plugin(
                self.override_install_path.clone(),
                self.client_config.clone(),
                self.plugin_opts.skip_update,
            )?;
            self.plugin_exe = Some(plugin_exe.clone());
            Ok(plugin_exe)
        }
    }

    pub fn get_command_to_spawn(&mut self) -> Result<String> {
        let plugin_exe = self.maybe_install_router()?;

        Ok(format!(
            "{} --supergraph {} --hot-reload --config {} --log {}",
            &plugin_exe,
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
            self.supergraph_opts.router_socket_addr()?
        );
        Ok(Fs::write_file(&self.router_config_path, contents, "")
            .context("could not create router config")?)
    }

    pub fn wait_for_startup(client: Client, port: &u16) -> Result<()> {
        let mut ready = false;
        let now = Instant::now();
        let seconds = 5;
        while !ready && now.elapsed() < Duration::from_secs(seconds) {
            let _ = client
                .get(format!(
                    "http://localhost:{}/.well-known/apollo/server-health",
                    port
                ))
                .send()
                .and_then(|r| r.error_for_status())
                .map(|_| {
                    ready = true;
                });
            std::thread::sleep(Duration::from_secs(1));
        }

        if ready {
            eprintln!(
                "router is running! head to http://localhost:{} to query your supergraph",
                port
            );
            Ok(())
        } else {
            Err(RoverError::new(anyhow!(
                "the router was unable to start up",
            )))
        }
    }

    pub fn wait_for_stop(client: Client, port: &u16) -> Result<()> {
        let mut ready = true;
        let now = Instant::now();
        let seconds = 5;
        while ready && now.elapsed() < Duration::from_secs(seconds) {
            let _ = client
                .get(format!(
                    "http://localhost:{}/.well-known/apollo/server-health",
                    port
                ))
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|_| {
                    ready = false;
                });
            std::thread::sleep(Duration::from_secs(1));
        }

        if !ready {
            tracing::info!("router stopped successfully");
            Ok(())
        } else {
            Err(RoverError::new(anyhow!("the router was unable to stop",)))
        }
    }

    pub fn spawn(&mut self) -> Result<()> {
        if self.router_handle.is_none() {
            let client = self.client_config.get_reqwest_client()?;
            self.write_router_config()?;
            self.maybe_install_router()?;
            self.router_handle = Some(BackgroundTask::new(self.get_command_to_spawn()?)?);
            Self::wait_for_startup(client, &self.supergraph_opts.port)
        } else {
            Ok(())
        }
    }

    pub fn kill(&mut self) -> Result<()> {
        tracing::info!("killing the router");
        if let Some(router_handle) = self.router_handle.as_mut() {
            router_handle.kill();
            self.router_handle = None;
            if let Ok(client) = self.client_config.get_reqwest_client() {
                let _ = Self::wait_for_stop(client, &self.supergraph_opts.port)
                    .map_err(log_err_and_continue);
            }
        }
        Ok(())
    }
}

impl Drop for RouterRunner {
    fn drop(&mut self) {
        let _ = self.kill().map_err(log_err_and_continue);
    }
}
