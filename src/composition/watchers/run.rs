use anyhow::Result;
use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::StreamExt;
use tokio::join;

use super::{
    handler::router_config::WriteRouterConfig,
    messages::receive_messages,
    messages::RoverDevMessage,
    subtask::{Subtask, SubtaskRunStream, SubtaskRunUnit},
    watcher::router_config::RouterConfigMessage,
    watcher::{file::FileWatcher, router_config::RouterConfigWatcher},
};

#[derive(Getters)]
pub struct RoverDevConfig {
    router: RoverDevRouterConfig,
}

#[derive(Getters)]
pub struct RoverDevRouterConfig {
    config_path: Utf8PathBuf,
    tmp_config_path: Utf8PathBuf,
}

pub async fn run(config: RoverDevConfig) -> Result<()> {
    let (router_config_messages, router_config_subtask) = Subtask::new(RouterConfigWatcher::new(
        FileWatcher::new(config.router.config_path().clone()),
    ));
    let write_router_config = WriteRouterConfig::new(config.router.tmp_config_path().clone());
    router_config_subtask.run();
    let mut messages = receive_messages(router_config_messages.boxed());
    let join_handle = tokio::spawn(async move {
        while let Some(message) = messages.next().await {
            match &message {
                RoverDevMessage::Config(RouterConfigMessage::Changed(contents)) => {
                    write_router_config.run(contents.as_str());
                }
            }
        }
    });
    join!(join_handle);
    Ok(())
}
