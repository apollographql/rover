use std::{
    fmt::{Display, Formatter},
    net::SocketAddr,
};

use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::StreamExt;
use regex::Regex;
use serde_yaml::Value;
use tap::TapFallible;

use crate::{subtask::SubtaskHandleStream, utils::effect::write_file::WriteFile};

use super::config::{parser::RouterConfigParser, RouterConfig};

use rover_std::{debugln, errln, infoln};

pub enum RouterUpdateEvent {
    SchemaChanged { schema: String },
    ConfigChanged { config: RouterConfig },
}

#[derive(Debug)]
pub enum HotReloadEvent {
    ConfigWritten(#[allow(unused)] Result<(), Box<dyn std::error::Error + Send>>),
    SchemaWritten(#[allow(unused)] Result<(), Box<dyn std::error::Error + Send>>),
}

#[derive(thiserror::Error, Debug)]
pub enum HotReloadError {
    #[error("Failed to parse the config")]
    Config {
        err: Box<dyn std::error::Error + Send + Sync>,
    },
}

#[derive(Builder, Debug, Copy, Clone)]
pub struct HotReloadConfigOverrides {
    pub address: SocketAddr,
}

#[derive(Builder)]
pub struct HotReloadWatcher<WriteF> {
    config: Utf8PathBuf,
    schema: Utf8PathBuf,
    write_file_impl: WriteF,
    overrides: HotReloadConfigOverrides,
}

pub struct HotReloadConfig {
    content: String,
}

impl HotReloadConfig {
    pub fn new(
        content: String,
        overrides: Option<HotReloadConfigOverrides>,
    ) -> Result<Self, HotReloadError> {
        match overrides {
            Some(overrides) => {
                let config = serde_yaml::from_str::<Value>(&content)
                    .map_err(|err| HotReloadError::Config { err: err.into() })?;

                // The config's address reflects the precedence logic (CLI override before config before
                // env before default), so we rely on whatever it gives us when passing it overrides
                let config_address = RouterConfigParser::new(&config, overrides.address)
                    .address()
                    .map_err(|err| HotReloadError::Config { err: err.into() })?
                    .to_string();

                let config = serde_yaml::to_string(&config).unwrap();

                let re = Regex::new(r"(?m)^  listen:.*$").expect("Failed to create Regex");
                let updated_config = re.replace(&config, format!("  listen: {config_address}"));

                Ok(Self {
                    content: updated_config.to_string(),
                })
            }
            None => Ok(Self { content }),
        }
    }
}

impl Display for HotReloadConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let config = &self.content;
        write!(f, "{config}")
    }
}

impl<WriteF> SubtaskHandleStream for HotReloadWatcher<WriteF>
where
    WriteF: WriteFile + Send + Clone + 'static,
{
    type Input = RouterUpdateEvent;
    type Output = HotReloadEvent;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
        mut input: futures::stream::BoxStream<'static, Self::Input>,
    ) -> tokio::task::AbortHandle {
        let write_file_impl = self.write_file_impl.clone();
        tokio::task::spawn(async move {
            while let Some(router_update_event) = input.next().await {
                match router_update_event {
                    RouterUpdateEvent::SchemaChanged { schema } => {
                        match write_file_impl
                            .write_file(&self.schema, schema.as_bytes())
                            .await
                        {
                            Ok(_) => {
                                let message = HotReloadEvent::SchemaWritten(Ok(()));
                                let _ = sender.send(message).tap_err(|err| {
                                    tracing::error!("Unable to send message. Error: {:?}", err)
                                });
                            }
                            Err(err) => {
                                let message = HotReloadEvent::SchemaWritten(Err(Box::new(err)));
                                let _ = sender.send(message).tap_err(|err| {
                                    tracing::error!("Unable to send message. Error: {:?}", err)
                                });
                            }
                        }
                    }
                    RouterUpdateEvent::ConfigChanged { config } => {
                        let hot_reload_config = match HotReloadConfig::new(
                            config.inner().to_string(),
                            Some(self.overrides),
                        ) {
                            Ok(config) => config,
                            Err(err) => {
                                let error_message =
                                    format!("Router config failed to update. {}", &err);
                                let message = HotReloadEvent::ConfigWritten(Err(Box::new(err)));
                                let _ = sender.send(message).tap_err(|err| {
                                    tracing::error!("Unable to send message. Error: {:?}", err)
                                });
                                errln!("{}", error_message);
                                break;
                            }
                        };

                        match write_file_impl
                            .write_file(&self.config, hot_reload_config.to_string().as_bytes())
                            .await
                        {
                            Ok(_) => {
                                let message = HotReloadEvent::ConfigWritten(Ok(()));
                                let _ = sender.send(message).tap_err(|err| {
                                    tracing::error!("Unable to send message. Error: {:?}", err)
                                });
                                infoln!("Router config updated.");
                                debugln!("{}", hot_reload_config);
                            }
                            Err(err) => {
                                let error_message =
                                    format!("Router config failed to update. {}", &err);
                                let message = HotReloadEvent::ConfigWritten(Err(Box::new(err)));
                                let _ = sender.send(message).tap_err(|err| {
                                    tracing::error!("Unable to send message. Error: {:?}", err)
                                });
                                errln!("{}", error_message);
                            }
                        }
                    }
                }
            }
        })
        .abort_handle()
    }
}
