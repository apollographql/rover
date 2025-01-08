use std::{
    fmt::{Display, Formatter},
    net::SocketAddr,
};

use anyhow::anyhow;
use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::StreamExt;
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

#[derive(Builder)]
pub struct HotReloadConfig {
    content: String,
    overrides: HotReloadConfigOverrides,
}

impl Display for HotReloadConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let yaml = serde_yaml::from_str::<Value>(&self.content).unwrap();
        let config_address = RouterConfigParser::new(&yaml, self.overrides.address)
            .address()
            .unwrap()
            .unwrap();

        // FIXME: blah
        let blah = serde_yaml::to_string(&yaml).unwrap();

        eprintln!("before update: {blah:?}");
        let updated_config = blah.replace(
            // FIXME: hardcoding 4000
            &format!("listen: 127.0.0.1:4000"),
            &format!("listen: {}", self.overrides.address.to_string()),
        );
        eprintln!("after update: {updated_config:?}");

        write!(f, "{updated_config}")
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
                        println!("HEEEELLO from routerupdateevent!");
                        let hot_reload_config = HotReloadConfig::builder()
                            .content(config.inner())
                            .overrides(self.overrides)
                            .build();

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
