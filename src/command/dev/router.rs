use apollo_federation_types::config::RouterVersion;
use saucer::{anyhow, Context, Fs, Utf8PathBuf};
use semver::Version;

use std::time::{Duration, Instant};

use crate::command::dev::command::BackgroundTask;
use crate::command::dev::SupergraphOpts;
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
        let plugin = Plugin::Router(RouterVersion::Exact(Version::parse("1.0.0-alpha.0")?));
        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepter: self.plugin_opts.elv2_license_accepter,
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
            self.supergraph_opts.supergraph_socket_addr()?
        );
        Ok(Fs::write_file(&self.router_config_path, contents, "")
            .context("could not create router config")?)
    }

    pub fn spawn(&mut self) -> Result<()> {
        if self.router_handle.is_none() {
            let client = self.client_config.get_reqwest_client()?;
            self.write_router_config()?;
            self.router_handle = Some(BackgroundTask::new(self.get_command_to_spawn()?)?);
            let mut ready = false;
            let now = Instant::now();
            let seconds = 5;
            while !ready && now.elapsed() < Duration::from_secs(seconds) {
                let _ = client
                    .get(format!(
                        "http://localhost:{}/.well-known/apollo/server-health",
                        &self.supergraph_opts.port
                    ))
                    .send()
                    .and_then(|r| r.error_for_status())
                    .map(|_| {
                        eprintln!(
                            "router is running! head to http://localhost:{} to query your supergraph",
                            &self.supergraph_opts.port
                        );
                    ready = true;
                    });
                std::thread::sleep(Duration::from_secs(1));
            }
            if ready {
                Ok(())
            } else {
                Err(RoverError::new(anyhow!(
                    "router did not start after {} seconds",
                    seconds
                )))
            }
        } else {
            Ok(())
        }
    }

    pub fn kill(&mut self) -> Result<()> {
        if let Some(router_handle) = self.router_handle.as_mut() {
            router_handle.kill();
            self.router_handle = None;
        }
        Ok(())
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
