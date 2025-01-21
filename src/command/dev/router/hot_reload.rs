use std::{
    fmt::{Display, Formatter},
    net::SocketAddr,
};

use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::StreamExt;
use rover_std::{debugln, errln, infoln};
use serde_yaml::Value;
use tap::TapFallible;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use super::config::{parser::RouterConfigParser, RouterConfig};
use crate::{subtask::SubtaskHandleStream, utils::effect::write_file::WriteFile};

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

#[derive(Debug)]
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
                let mut config = serde_yaml::from_str::<Value>(&content)
                    .map_err(|err| HotReloadError::Config { err: err.into() })?;

                // The config's address reflects the precedence logic (CLI override before config before
                // env before default), so we rely on whatever it gives us when passing it overrides
                let processed_address = RouterConfigParser::new(&config, overrides.address)
                    .address()
                    .map_err(|err| HotReloadError::Config { err: err.into() })?
                    .to_string();

                let processed_address =
                    serde_yaml::to_value(&processed_address).map_err(|err| {
                        HotReloadError::Config {
                            err: format!("Failed to parse router config: {err}").into(),
                        }
                    })?;

                let addr = config
                    .get_mut("supergraph")
                    .and_then(|sup| sup.get_mut("listen"))
                    .ok_or(HotReloadError::Config {
                        err: "Failed to parse router config's supergraph.listen field"
                            .to_string()
                            .into(),
                    })
                    .tap_err(|err| debug!("{err}"))?;

                *addr = processed_address;

                let config = serde_yaml::to_string(&config)
                    .map_err(|err| HotReloadError::Config { err: err.into() })?;

                Ok(Self {
                    content: config.to_string(),
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
        cancellation_token: Option<CancellationToken>,
    ) {
        let write_file_impl = self.write_file_impl.clone();
        let cancellation_token = cancellation_token.unwrap_or_default();
        tokio::task::spawn(async move {
            cancellation_token
                .run_until_cancelled(async move {
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
                                            tracing::error!(
                                                "Unable to send message. Error: {:?}",
                                                err
                                            )
                                        });
                                    }
                                    Err(err) => {
                                        let message =
                                            HotReloadEvent::SchemaWritten(Err(Box::new(err)));
                                        let _ = sender.send(message).tap_err(|err| {
                                            tracing::error!(
                                                "Unable to send message. Error: {:?}",
                                                err
                                            )
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
                                        let message =
                                            HotReloadEvent::ConfigWritten(Err(Box::new(err)));
                                        let _ = sender.send(message).tap_err(|err| {
                                            tracing::error!(
                                                "Unable to send message. Error: {:?}",
                                                err
                                            )
                                        });
                                        errln!("{}", error_message);
                                        break;
                                    }
                                };

                                match write_file_impl
                                    .write_file(
                                        &self.config,
                                        hot_reload_config.to_string().as_bytes(),
                                    )
                                    .await
                                {
                                    Ok(_) => {
                                        let message = HotReloadEvent::ConfigWritten(Ok(()));
                                        let _ = sender.send(message).tap_err(|err| {
                                            tracing::error!(
                                                "Unable to send message. Error: {:?}",
                                                err
                                            )
                                        });
                                        infoln!("Router config updated.");
                                        debugln!("{}", hot_reload_config);
                                    }
                                    Err(err) => {
                                        let error_message =
                                            format!("Router config failed to update. {}", &err);
                                        let message =
                                            HotReloadEvent::ConfigWritten(Err(Box::new(err)));
                                        let _ = sender.send(message).tap_err(|err| {
                                            tracing::error!(
                                                "Unable to send message. Error: {:?}",
                                                err
                                            )
                                        });
                                        errln!("{}", error_message);
                                    }
                                }
                            }
                        }
                    }
                })
                .await;
        });
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;

    #[fixture]
    fn router_config() -> &'static str {
        indoc::indoc! { r#"
supergraph:
  listen: 127.0.0.1:4000
telemetry:
  instrumentation:
    spans:
      mode: spec_compliant
health_check:
  enabled: true
headers:
  all:
    request:
      - propagate:
          matching: .*
"#
        }
    }

    // NB: serde_yaml formats what we give it; below represents the above, with an address override
    // applied and having been passed through serde_yaml (notice 15 lines down, where the
    // indendation differs between the two yamls)
    #[fixture]
    fn router_config_expectation() -> &'static str {
        indoc::indoc! { r#"
supergraph:
  listen: 127.0.0.1:8888
telemetry:
  instrumentation:
    spans:
      mode: spec_compliant
health_check:
  enabled: true
headers:
  all:
    request:
    - propagate:
        matching: .*
"#
        }
    }

    #[rstest]
    fn overrides_apply(router_config: &'static str, router_config_expectation: &'static str) {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8888);
        let overrides = HotReloadConfigOverrides::new(address);
        let hot_reload_config = HotReloadConfig::new(router_config.to_string(), Some(overrides));
        assert_that!(hot_reload_config).is_ok().matches(|config| {
            println!("{config}");
            println!("{router_config_expectation}");

            config.to_string() == router_config_expectation
        });
    }
}
