use camino::Utf8PathBuf;
use dotenvy::dotenv_override;
use futures::{StreamExt, stream::BoxStream};
use rover_std::Fs;
use tap::TapFallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::{
    command::dev::router::{
        config::RouterConfig, hot_reload::RouterUpdateEvent, watchers::file::FileWatcher,
    },
    subtask::{SubtaskHandleStream, SubtaskHandleUnit},
};

pub struct DotEnvWatcher {
    file_watcher: FileWatcher,
}

#[derive(Debug)]
pub enum DotEnvEvent {
    DotEnvChanged,
}

impl DotEnvWatcher {
    pub fn new() -> Self {
        let dot_env_path = dotenvy::dotenv()
            .ok()
            .and_then(|p| Utf8PathBuf::from_path_buf(p).ok())
            .unwrap_or_else(|| Utf8PathBuf::from(".env"));

        Self {
            file_watcher: FileWatcher::new(dot_env_path),
        }
    }
}

impl SubtaskHandleUnit for DotEnvWatcher {
    type Output = DotEnvEvent;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
        cancellation_token: Option<CancellationToken>,
    ) {
        let cancellation_token = cancellation_token.unwrap_or_default();
        tokio::spawn(async move {
            cancellation_token
                .run_until_cancelled(async move {
                    // Emit a DotEnvEvent::Changed event which will trigger a router config reload
                    while let Some(_env_contents) = self.file_watcher.clone().watch().next().await {
                        let _ = sender
                            .send(DotEnvEvent::DotEnvChanged)
                            .tap_err(|err| tracing::error!("{:?}", err));
                    }
                })
                .await;
        });
    }
}

pub struct DotEnvReload {
    pub router_config_path: Utf8PathBuf,
}

impl SubtaskHandleStream for DotEnvReload {
    type Input = DotEnvEvent;

    type Output = RouterUpdateEvent;

    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
        cancellation_token: Option<CancellationToken>,
    ) {
        let cancellation_token = cancellation_token.unwrap_or_default();
        tokio::spawn(async move {
            cancellation_token
                .run_until_cancelled(async move {
                    while let Some(_evt) = input.next().await {
                        dotenv_override().ok();
                        match Fs::read_file(self.router_config_path.clone()) {
                            Ok(config_contents) => {
                                let _ = sender
                                    .send(RouterUpdateEvent::ConfigChanged {
                                        config: RouterConfig::new(config_contents),
                                    })
                                    .tap_err(|err| tracing::error!("{:?}", err));
                            }
                            Err(err) => {
                                tracing::error!(
                                    "Could not read router config after .env change: {:?}",
                                    err
                                );
                            }
                        }
                    }
                })
                .await;
        });
    }
}
