use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::StreamExt;
use tap::TapFallible;

use crate::{subtask::SubtaskHandleStream, utils::effect::write_file::WriteFile};

use super::config::RouterConfig;

pub enum RouterUpdateEvent {
    SchemaChanged { schema: String },
    ConfigChanged { config: RouterConfig },
}

#[derive(Debug)]
pub enum HotReloadEvent {
    ConfigWritten(#[allow(unused)] Result<(), Box<dyn std::error::Error + Send>>),
    SchemaWritten(#[allow(unused)] Result<(), Box<dyn std::error::Error + Send>>),
}

#[derive(Builder)]
pub struct HotReloadWatcher<WriteF> {
    config: Utf8PathBuf,
    schema: Utf8PathBuf,
    write_file_impl: WriteF,
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
                        match write_file_impl
                            .write_file(&self.config, config.inner().as_bytes())
                            .await
                        {
                            Ok(_) => {
                                let message = HotReloadEvent::ConfigWritten(Ok(()));
                                let _ = sender.send(message).tap_err(|err| {
                                    tracing::error!("Unable to send message. Error: {:?}", err)
                                });
                            }
                            Err(err) => {
                                let message = HotReloadEvent::ConfigWritten(Err(Box::new(err)));
                                let _ = sender.send(message).tap_err(|err| {
                                    tracing::error!("Unable to send message. Error: {:?}", err)
                                });
                            }
                        }
                    }
                }
            }
        })
        .abort_handle()
    }
}
