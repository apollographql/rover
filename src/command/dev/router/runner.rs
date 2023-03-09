use anyhow::{anyhow, Context};
use apollo_federation_types::config::RouterVersion;
use camino::Utf8PathBuf;
use crossbeam_channel::bounded;
use reqwest::blocking::Client;
use reqwest::Url;
use rover_std::{Emoji, Style};
use semver::Version;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::command::dev::{
    do_dev::log_err_and_continue,
    router::{BackgroundTask, BackgroundTaskLog},
    OVERRIDE_DEV_ROUTER_VERSION,
};
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
    router_listen_path: String,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    plugin_exe: Option<Utf8PathBuf>,
    router_handle: Option<BackgroundTask>,
}

impl RouterRunner {
    pub fn new(
        supergraph_schema_path: Utf8PathBuf,
        router_config_path: Utf8PathBuf,
        plugin_opts: PluginOpts,
        router_socket_addr: SocketAddr,
        router_listen_path: String,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Self {
        Self {
            supergraph_schema_path,
            router_config_path,
            plugin_opts,
            router_socket_addr,
            router_listen_path,
            override_install_path,
            client_config,
            router_handle: None,
            plugin_exe: None,
        }
    }

    fn install_command(&self) -> RoverResult<Install> {
        let plugin = match &*OVERRIDE_DEV_ROUTER_VERSION {
            Some(version) => Plugin::Router(RouterVersion::Exact(Version::parse(version)?)),
            None => Plugin::Router(RouterVersion::Latest),
        };
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
        Ok(format!(
            "{plugin_exe} --supergraph {supergraph} --hot-reload --config {config} --log trace --dev",
            plugin_exe = self.maybe_install_router()?,
            supergraph = self.supergraph_schema_path.as_str(),
            config = self.router_config_path.as_str(),
        ))
    }

    pub fn wait_for_startup(&mut self, client: Client) -> RoverResult<()> {
        let mut ready = false;
        let now = Instant::now();
        let seconds = 5;
        let base_url = format!(
            "http://{}{}",
            &self.router_socket_addr, &self.router_listen_path
        );
        let mut endpoint =
            Url::parse(&base_url).with_context(|| format!("{base_url} is not a valid URL."))?;
        endpoint.set_query(Some("query={__typename}"));
        let endpoint = endpoint.to_string();
        while !ready && now.elapsed() < Duration::from_secs(seconds) {
            let _ = client
                .get(&endpoint)
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
                "{}your supergraph is running! head to http://{}{} to query your supergraph",
                Emoji::Rocket,
                &self
                    .router_socket_addr
                    .to_string()
                    .replace("127.0.0.1", "localhost")
                    .replace("0.0.0.0", "localhost")
                    .replace("[::]", "localhost")
                    .replace("[::1]", "localhost"),
                &self.router_listen_path
            );
            Ok(())
        } else {
            Err(RoverError::new(anyhow!(
                "the router was unable to start up",
            )))
        }
    }

    pub fn wait_for_stop(&mut self, client: Client) -> RoverResult<()> {
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
            self.maybe_install_router()?;
            let (router_log_sender, router_log_receiver) = bounded(0);
            let router_handle =
                BackgroundTask::new(self.get_command_to_spawn()?, router_log_sender)?;
            tracing::info!("spawning router with `{}`", router_handle.descriptor());

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

            self.wait_for_startup(client)?;
            self.router_handle = Some(router_handle);

            Ok(())
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
