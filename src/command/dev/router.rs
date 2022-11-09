use anyhow::{anyhow, Context};
use apollo_federation_types::config::RouterVersion;
use camino::Utf8PathBuf;
use crossbeam_channel::bounded as sync_channel;
use reqwest::blocking::Client;
use rover_std::{Emoji, Fs, Style};
use semver::Version;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::command::dev::command::{BackgroundTask, BackgroundTaskLog};
use crate::command::dev::do_dev::log_err_and_continue;
use crate::command::dev::DEV_ROUTER_VERSION;
use crate::command::install::Plugin;
use crate::command::Install;
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverResult};

#[derive(Debug)]
pub struct RouterRunner {
    supergraph_schema_path: Utf8PathBuf,
    router_config_path: Utf8PathBuf,
    plugin_opts: PluginOpts,
    router_socket_addr: SocketAddr,
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
        router_socket_addr: SocketAddr,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Self {
        Self {
            supergraph_schema_path,
            router_config_path,
            plugin_opts,
            router_socket_addr,
            override_install_path,
            client_config,
            router_handle: None,
            plugin_exe: None,
        }
    }

    fn install_command(&self) -> RoverResult<Install> {
        let plugin = Plugin::Router(RouterVersion::Exact(Version::parse(&DEV_ROUTER_VERSION)?));
        Ok(Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepter: self.plugin_opts.elv2_license_accepter,
        })
    }

    pub fn maybe_install_router(&mut self) -> RoverResult<Utf8PathBuf> {
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

    pub fn get_command_to_spawn(&mut self) -> RoverResult<String> {
        let plugin_exe = self.maybe_install_router()?;

        Ok(format!(
            "{} --supergraph {} --hot-reload --config {} --log trace --dev",
            &plugin_exe,
            self.supergraph_schema_path.as_str(),
            self.router_config_path.as_str(),
        ))
    }

    fn write_router_config(&self) -> RoverResult<()> {
        let contents = format!(
            r#"
        supergraph:
          listen: '{0}'
        "#,
            &self.router_socket_addr
        );
        Ok(Fs::write_file(&self.router_config_path, contents)
            .context("could not create router config")?)
    }

    pub fn wait_for_startup(&self, client: Client) -> RoverResult<()> {
        let mut ready = false;
        let now = Instant::now();
        let seconds = 5;
        while !ready && now.elapsed() < Duration::from_secs(seconds) {
            let _ = client
                .get(format!(
                    "http://{}/?query={{__typename}}",
                    &self.router_socket_addr
                ))
                .header("Content-Type", "application/json")
                .send()
                .and_then(|r| r.error_for_status())
                .map(|_| {
                    ready = true;
                });
            std::thread::sleep(Duration::from_millis(250));
        }

        if ready {
            eprintln!(
                "{}your supergraph is running! head to http://{} to query your supergraph",
                Emoji::Rocket,
                &self
                    .router_socket_addr
                    .to_string()
                    .replace("127.0.0.1", "localhost")
                    .replace("0.0.0.0", "localhost")
                    .replace("[::]", "localhost")
                    .replace("[::1]", "localhost")
            );
            Ok(())
        } else {
            Err(RoverError::new(anyhow!(
                "the router was unable to start up",
            )))
        }
    }

    pub fn wait_for_stop(&self, client: Client) -> RoverResult<()> {
        let mut ready = true;
        let now = Instant::now();
        let seconds = 5;
        while ready && now.elapsed() < Duration::from_secs(seconds) {
            let _ = client
                .get(format!(
                    "http://{}/?query={{__typename}}",
                    &self.router_socket_addr
                ))
                .header("Content-Type", "application/json")
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|_| {
                    ready = false;
                });
            std::thread::sleep(Duration::from_millis(250));
        }

        if !ready {
            tracing::info!("router stopped successfully");
            Ok(())
        } else {
            Err(RoverError::new(anyhow!("the router was unable to stop",)))
        }
    }

    pub fn spawn(&mut self) -> RoverResult<()> {
        if self.router_handle.is_none() {
            let client = self.client_config.get_reqwest_client()?;
            self.write_router_config()?;
            self.maybe_install_router()?;
            let (router_log_sender, router_log_receiver) = sync_channel(0);
            self.router_handle = Some(BackgroundTask::new(
                self.get_command_to_spawn()?,
                router_log_sender,
            )?);
            rayon::spawn(move || loop {
                if let Ok(BackgroundTaskLog::Stdout(stdout)) = router_log_receiver.recv() {
                    if let Ok(stdout) = serde_json::from_str::<serde_json::Value>(&stdout) {
                        let fields = &stdout["fields"];
                        if let Some(level) = stdout["level"].as_str() {
                            if let Some(message) = fields["message"].as_str() {
                                let warn_prefix = Style::WarningPrefix.paint("WARN:");
                                let error_prefix = Style::ErrorPrefix.paint("ERROR:");
                                if let Some(router_span) = stdout["target"].as_str() {
                                    match level {
                                        "INFO" => tracing::info!(%message, %router_span),
                                        "DEBUG" => tracing::debug!(%message, %router_span),
                                        "TRACE" => tracing::trace!(%message, %router_span),
                                        "WARN" => eprintln!("{} {}", warn_prefix, &message),
                                        "ERROR" => {
                                            eprintln!("{} {}", error_prefix, &message)
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            });
            self.wait_for_startup(client)
        } else {
            Ok(())
        }
    }

    pub fn kill(&mut self) -> RoverResult<()> {
        if self.router_handle.is_some() {
            tracing::info!("killing the router");
            self.router_handle = None;
            if let Ok(client) = self.client_config.get_reqwest_client() {
                let _ = self.wait_for_stop(client).map_err(log_err_and_continue);
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
