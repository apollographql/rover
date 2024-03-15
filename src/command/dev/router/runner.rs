use anyhow::{anyhow, Context};
use apollo_federation_types::config::RouterVersion;
use camino::Utf8PathBuf;
use crossbeam_channel::bounded;
use reqwest::Client;
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

    pub async fn maybe_install_router(&mut self) -> RoverResult<Utf8PathBuf> {
        if let Some(plugin_exe) = &self.plugin_exe {
            Ok(plugin_exe.clone())
        } else {
            let install_command = self.install_command()?;
            let plugin_exe = install_command
                .get_versioned_plugin(
                    self.override_install_path.clone(),
                    self.client_config.clone(),
                    self.plugin_opts.skip_update,
                )
                .await?;
            self.plugin_exe = Some(plugin_exe.clone());
            Ok(plugin_exe)
        }
    }

    pub async fn get_command_to_spawn(&mut self) -> RoverResult<String> {
        Ok(format!(
            "{plugin_exe} --supergraph {supergraph} --hot-reload --config {config} --log trace --dev",
            plugin_exe = self.maybe_install_router().await?,
            supergraph = self.supergraph_schema_path.as_str(),
            config = self.router_config_path.as_str(),
        ))
    }

    pub async fn wait_for_startup(&mut self, client: Client) -> RoverResult<()> {
        let mut ready = false;
        let now = Instant::now();
        let seconds = 10;
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
                .await
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

    pub async fn wait_for_stop(&mut self, client: Client) -> RoverResult<()> {
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
                .await
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

    pub async fn spawn(&mut self) -> RoverResult<()> {
        if self.router_handle.is_none() {
            let client = self.client_config.get_reqwest_client()?;
            self.maybe_install_router().await?;
            let (router_log_sender, router_log_receiver) = bounded(0);
            let router_handle = BackgroundTask::new(
                self.get_command_to_spawn().await?,
                router_log_sender,
                &self.client_config,
                &self.plugin_opts.profile,
            )
            .await?;
            tracing::info!("spawning router with `{}`", router_handle.descriptor());

            let warn_prefix = Style::WarningPrefix.paint("WARN:");
            let error_prefix = Style::ErrorPrefix.paint("ERROR:");
            let unknown_prefix = Style::ErrorPrefix.paint("UNKNOWN:");
            // Build a Rayon Thread pool
            let tp = rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .thread_name(|idx| format!("router-runner-{idx}"))
                .build()
                .map_err(|err| {
                    RoverError::new(anyhow!("could not create router runner thread pool: {err}",))
                })?;
            tp.spawn(move || loop {
                while let Ok(log) = router_log_receiver.recv() {
                    match log {
                        BackgroundTaskLog::Stdout(stdout) => {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stdout) {
                                let fields = &parsed["fields"];
                                let level = parsed["level"].as_str().unwrap_or("UNKNOWN");
                                let message = fields["message"]
                                    .as_str()
                                    .or_else(|| {
                                        // Message is in a slightly different location depending on the
                                        // version of Router
                                        parsed["message"].as_str()
                                    })
                                    .unwrap_or(&stdout);

                                match level {
                                    "INFO" => tracing::info!(%message),
                                    "DEBUG" => tracing::debug!(%message),
                                    "TRACE" => tracing::trace!(%message),
                                    "WARN" => eprintln!("{} {}", warn_prefix, &message),
                                    "ERROR" => {
                                        eprintln!("{} {}", error_prefix, &message)
                                    }
                                    "UNKNOWN" => {
                                        eprintln!("{} {}", unknown_prefix, &message)
                                    }
                                    _ => {}
                                }
                            } else {
                                eprintln!("{} {}", warn_prefix, &stdout)
                            }
                        }
                        BackgroundTaskLog::Stderr(stderr) => {
                            eprintln!("{} {}", error_prefix, &stderr)
                        }
                    };
                }
            });

            self.wait_for_startup(client).await?;
            self.router_handle = Some(router_handle);

            Ok(())
        } else {
            Ok(())
        }
    }

    pub async fn kill(&mut self) -> RoverResult<()> {
        if self.router_handle.is_some() {
            tracing::info!("killing the router");
            self.router_handle = None;
            if let Ok(client) = self.client_config.get_reqwest_client() {
                let _ = self
                    .wait_for_stop(client)
                    .await
                    .map_err(log_err_and_continue);
            }
        }
        Ok(())
    }
}

impl Drop for RouterRunner {
    fn drop(&mut self) {
        let router_handle = self.router_handle.take();
        let client_config = self.client_config.clone();
        let router_socket_addr = self.router_socket_addr;
        // copying the kill procedure here to emulate an async drop
        tokio::task::spawn(async move {
            if router_handle.is_some() {
                tracing::info!("killing the router");
                if let Ok(client) = client_config.get_reqwest_client() {
                    let mut ready = true;
                    let now = Instant::now();
                    let seconds = 5;
                    while ready && now.elapsed() < Duration::from_secs(seconds) {
                        let _ = client
                            .get(format!(
                                "http://{}/?query={{__typename}}",
                                &router_socket_addr
                            ))
                            .header("Content-Type", "application/json")
                            .send()
                            .await
                            .and_then(|r| r.error_for_status())
                            .map_err(|_| {
                                ready = false;
                            });
                        std::thread::sleep(Duration::from_millis(250));
                    }

                    if !ready {
                        tracing::info!("router stopped successfully");
                    } else {
                        log_err_and_continue(RoverError::new(anyhow!(
                            "the router was unable to stop",
                        )));
                    }
                }
            }
        });
    }
}
